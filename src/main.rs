use actix_web::{middleware::Logger, web, App, HttpServer};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod api;
mod auth;
mod config;
mod db;
mod entities;
mod jobs;
mod kafka;
mod models;
mod services;

use config::Config;
use db::{create_mongodb_client, create_mysql_pool, create_redis_client};
use jobs::{calculate_top_stats, handle_user_created_event};
use kafka::{parse_feed_event, FeedEventType, KafkaConsumer, KafkaProducer};
use services::notification::{
    handle_feed_commented_event, handle_feed_liked_event, handle_feed_viewed_event,
};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let config = Config::from_env().expect("Failed to load configuration");

    log::info!(
        "Starting server on {}:{}",
        config.server.host,
        config.server.port
    );
    let mysql_pool = create_mysql_pool(&config)
        .await
        .expect("Failed to create MySQL pool");

    let mongodb_db = create_mongodb_client(&config)
        .await
        .expect("Failed to create MongoDB client");

    let redis_client = create_redis_client(&config).expect("Failed to create Redis client");

    log::info!("Database connections established");

    let kafka_producer = KafkaProducer::new(&config).expect("Failed to create Kafka producer");
    let kafka_consumer_user = KafkaConsumer::new(&config, vec!["user_events".to_string()])
        .expect("Failed to create Kafka consumer");

    kafka_consumer_user
        .subscribe()
        .await
        .expect("Failed to subscribe to Kafka topics");

    kafka_consumer_user
        .start_consuming(|topic, key, payload| match topic.as_str() {
            "user_events" => {
                handle_user_created_event(topic, key, payload);
            }
            _ => {
                log::warn!("Unknown topic: {}", topic);
            }
        })
        .await
        .expect("Failed to start Kafka consumer");

    let mysql_pool_clone = mysql_pool.clone();
    let mongodb_db_clone = mongodb_db.clone();
    let redis_client_clone = redis_client.clone();
    let kafka_consumer_feed = KafkaConsumer::new(&config, vec!["feed_events".to_string()])
        .expect("Failed to create Kafka consumer for feed events");

    kafka_consumer_feed
        .subscribe()
        .await
        .expect("Failed to subscribe to feed events");

    kafka_consumer_feed
        .start_consuming(move |topic, _key, payload| {
            if topic == "feed_events" {
                match std::str::from_utf8(&payload) {
                    Ok(payload_str) => {
                        log::debug!("Received feed event payload: {}", payload_str);
                        match parse_feed_event(payload_str) {
                            Ok((event_type, event_data)) => {
                                log::info!(
                                    "Parsed feed event: {:?}, data: {:?}",
                                    event_type,
                                    event_data
                                );
                                let mysql_pool = mysql_pool_clone.clone();
                                let mongo_db = mongodb_db_clone.clone();
                                let redis_client = redis_client_clone.clone();

                                tokio::spawn(async move {
                                    match event_type {
                                        FeedEventType::Liked => {
                                            handle_feed_liked_event(
                                                &event_data,
                                                &mongo_db,
                                                &mysql_pool,
                                                &redis_client,
                                            )
                                            .await;
                                        }
                                        FeedEventType::Commented => {
                                            log::info!("Received commented event, processing...");
                                            handle_feed_commented_event(
                                                &event_data,
                                                &mongo_db,
                                                &mysql_pool,
                                                &redis_client,
                                            )
                                            .await;
                                            log::info!("Finished processing commented event");
                                        }
                                        FeedEventType::Viewed => {
                                            handle_feed_viewed_event(&event_data, &redis_client)
                                                .await;
                                        }
                                        FeedEventType::Created => {
                                            log::info!("Feed created event received (no handler)");
                                        }
                                    }
                                });
                            }
                            Err(e) => {
                                log::error!("Failed to parse feed event: {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to decode feed event: {:?}", e);
                    }
                }
            }
        })
        .await
        .expect("Failed to start feed events consumer");

    log::info!("Kafka consumers started");

    let mysql_pool_job = mysql_pool.clone();
    let mongodb_db_job = mongodb_db.clone();
    let redis_client_job = redis_client.clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            log::info!("Calculating top stats...");
            calculate_top_stats(&mysql_pool_job, &mongodb_db_job, &redis_client_job).await;
        }
    });

    let mysql_pool_init = mysql_pool.clone();
    let mongodb_db_init = mongodb_db.clone();
    let redis_client_init = redis_client.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        log::info!("Calculating initial top stats...");
        calculate_top_stats(&mysql_pool_init, &mongodb_db_init, &redis_client_init).await;
    });

    let openapi = api::ApiDoc::openapi();

    let server_host = config.server.host.clone();
    let server_port = config.server.port;
    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .app_data(web::Data::new(config.clone()))
            .app_data(web::Data::new(mysql_pool.clone()))
            .app_data(web::Data::new(mongodb_db.clone()))
            .app_data(web::Data::new(redis_client.clone()))
            .app_data(web::Data::new(kafka_producer.clone()))
            .route(
                "/api/docs",
                web::get().to(|| async {
                    actix_web::HttpResponse::PermanentRedirect()
                        .append_header(("Location", "/api/docs/"))
                        .finish()
                }),
            )
            .service(
                SwaggerUi::new("/api/docs/{_:.*}").url("/api-docs/openapi.json", openapi.clone()),
            )
            .service(
                web::scope("/api")
                    .service(
                        web::scope("/auth")
                            .route("/signup", web::post().to(api::auth::signup))
                            .route("/login", web::post().to(api::auth::login)),
                    )
                    .service(
                        web::scope("/feed")
                            .route("", web::post().to(api::feed::create_feed))
                            .route("", web::get().to(api::feed::get_feeds))
                            .route("/{feed_id}/like", web::post().to(api::feed::like_feed))
                            .route("/{feed_id}/like", web::delete().to(api::feed::unlike_feed))
                            .route(
                                "/{feed_id}/comment",
                                web::post().to(api::feed::comment_feed),
                            )
                            .route(
                                "/{feed_id}/comments",
                                web::get().to(api::feed::get_comments),
                            )
                            .route("/{feed_id}/view", web::post().to(api::feed::view_feed)),
                    )
                    .service(
                        web::scope("/notify")
                            .route("", web::get().to(api::notify::get_notifications))
                            .route(
                                "/{notification_id}/read",
                                web::put().to(api::notify::mark_notification_read),
                            ),
                    )
                    .service(
                        web::scope("/top")
                            .route("/users-liked", web::get().to(api::top::get_top_users_liked))
                            .route(
                                "/feeds-commented",
                                web::get().to(api::top::get_top_comments),
                            )
                            .route(
                                "/feeds-viewed",
                                web::get().to(api::top::get_top_feeds_viewed),
                            )
                            .route("/feeds-liked", web::get().to(api::top::get_top_feeds_liked)),
                    ),
            )
    })
    .bind(format!("{}:{}", server_host, server_port))?
    .run()
    .await
}
