#!/bin/bash

# API Test Bot Script
# Tests all endpoints of the Rust Web Service

set -e

BASE_URL="${BASE_URL:-http://localhost:8080}"
API_URL="${BASE_URL}/api"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test counters
TESTS_PASSED=0
TESTS_FAILED=0

# Helper function to print test results
print_test() {
    local test_name=$1
    local status=$2
    local message=$3
    
    if [ "$status" = "PASS" ]; then
        echo -e "${GREEN}✓${NC} $test_name"
        ((TESTS_PASSED++))
    else
        echo -e "${RED}✗${NC} $test_name: $message"
        ((TESTS_FAILED++))
    fi
}

# Helper function to make API calls
api_call() {
    local method=$1
    local endpoint=$2
    local data=$3
    local token=$4
    
    local headers=()
    if [ -n "$token" ]; then
        headers+=(-H "Authorization: Bearer $token")
    fi
    headers+=(-H "Content-Type: application/json")
    
    if [ -n "$data" ]; then
        curl -s -w "\n%{http_code}" -X "$method" "${API_URL}${endpoint}" \
            "${headers[@]}" \
            -d "$data"
    else
        curl -s -w "\n%{http_code}" -X "$method" "${API_URL}${endpoint}" \
            "${headers[@]}"
    fi
}

echo "========================================="
echo "API Test Bot - Rust Web Service"
echo "========================================="
echo "Base URL: $BASE_URL"
echo ""

# Test 1: Health check (if available) or get feeds (public endpoint)
echo "Testing public endpoints..."
RESPONSE=$(api_call GET "/feed" "" "")
HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
BODY=$(echo "$RESPONSE" | sed '$d')

if [ "$HTTP_CODE" = "200" ]; then
    print_test "GET /api/feed (public)" "PASS"
else
    print_test "GET /api/feed (public)" "FAIL" "HTTP $HTTP_CODE"
fi

# Test 2: Signup
echo ""
echo "Testing authentication..."
SIGNUP_DATA='{"email":"test@example.com","username":"testuser","password":"testpass123"}'
RESPONSE=$(api_call POST "/auth/signup" "$SIGNUP_DATA" "")
HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
BODY=$(echo "$RESPONSE" | sed '$d')

if [ "$HTTP_CODE" = "200" ]; then
    TOKEN=$(echo "$BODY" | grep -o '"token":"[^"]*' | cut -d'"' -f4)
    if [ -n "$TOKEN" ]; then
        print_test "POST /api/auth/signup" "PASS"
        USER_ID=$(echo "$BODY" | grep -o '"id":[0-9]*' | head -1 | cut -d':' -f2)
    else
        print_test "POST /api/auth/signup" "FAIL" "No token in response"
    fi
else
    print_test "POST /api/auth/signup" "FAIL" "HTTP $HTTP_CODE: $BODY"
    echo "Attempting login with existing user..."
    LOGIN_DATA='{"email":"test@example.com","password":"testpass123"}'
    RESPONSE=$(api_call POST "/auth/login" "$LOGIN_DATA" "")
    HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
    BODY=$(echo "$RESPONSE" | sed '$d')
    if [ "$HTTP_CODE" = "200" ]; then
        TOKEN=$(echo "$BODY" | grep -o '"token":"[^"]*' | cut -d'"' -f4)
        if [ -n "$TOKEN" ]; then
            print_test "POST /api/auth/login" "PASS"
        fi
    fi
fi

if [ -z "$TOKEN" ]; then
    echo -e "${RED}Error: Could not obtain authentication token. Exiting.${NC}"
    exit 1
fi

# Test 3: Create Feed
echo ""
echo "Testing feed endpoints..."
FEED_DATA='{"content":"This is a test feed from API bot"}'
RESPONSE=$(api_call POST "/feed" "$FEED_DATA" "$TOKEN")
HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
BODY=$(echo "$RESPONSE" | sed '$d')

if [ "$HTTP_CODE" = "200" ]; then
    FEED_ID=$(echo "$BODY" | grep -o '"id":[0-9]*' | head -1 | cut -d':' -f2)
    print_test "POST /api/feed" "PASS"
else
    print_test "POST /api/feed" "FAIL" "HTTP $HTTP_CODE: $BODY"
    FEED_ID="1"
fi

# Test 4: Get Feeds
RESPONSE=$(api_call GET "/feed?limit=10&offset=0" "" "$TOKEN")
HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
if [ "$HTTP_CODE" = "200" ]; then
    print_test "GET /api/feed (authenticated)" "PASS"
else
    print_test "GET /api/feed (authenticated)" "FAIL" "HTTP $HTTP_CODE"
fi

# Test 5: Like Feed
RESPONSE=$(api_call POST "/feed/${FEED_ID}/like" "" "$TOKEN")
HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
if [ "$HTTP_CODE" = "200" ]; then
    print_test "POST /api/feed/{id}/like" "PASS"
else
    print_test "POST /api/feed/{id}/like" "FAIL" "HTTP $HTTP_CODE"
fi

# Test 6: View Feed
RESPONSE=$(api_call POST "/feed/${FEED_ID}/view" "" "$TOKEN")
HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
if [ "$HTTP_CODE" = "200" ]; then
    print_test "POST /api/feed/{id}/view" "PASS"
else
    print_test "POST /api/feed/{id}/view" "FAIL" "HTTP $HTTP_CODE"
fi

# Test 7: Comment on Feed
COMMENT_DATA='{"content":"This is a test comment"}'
RESPONSE=$(api_call POST "/feed/${FEED_ID}/comment" "$COMMENT_DATA" "$TOKEN")
HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
if [ "$HTTP_CODE" = "200" ]; then
    print_test "POST /api/feed/{id}/comment" "PASS"
else
    print_test "POST /api/feed/{id}/comment" "FAIL" "HTTP $HTTP_CODE"
fi

# Test 8: Get Comments
RESPONSE=$(api_call GET "/feed/${FEED_ID}/comments" "" "")
HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
if [ "$HTTP_CODE" = "200" ]; then
    print_test "GET /api/feed/{id}/comments" "PASS"
else
    print_test "GET /api/feed/{id}/comments" "FAIL" "HTTP $HTTP_CODE"
fi

# Test 9: Get Notifications
RESPONSE=$(api_call GET "/notify?limit=10" "" "$TOKEN")
HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
if [ "$HTTP_CODE" = "200" ]; then
    print_test "GET /api/notify" "PASS"
    NOTIFICATION_ID=$(echo "$RESPONSE" | sed '$d' | grep -o '"_id":"[^"]*' | head -1 | cut -d'"' -f4)
    if [ -z "$NOTIFICATION_ID" ]; then
        NOTIFICATION_ID=$(echo "$RESPONSE" | sed '$d' | grep -o '"id":"[^"]*' | head -1 | cut -d'"' -f4)
    fi
else
    print_test "GET /api/notify" "FAIL" "HTTP $HTTP_CODE"
fi

# Test 10: Mark Notification as Read (if we have one)
if [ -n "$NOTIFICATION_ID" ]; then
    RESPONSE=$(api_call PUT "/notify/${NOTIFICATION_ID}/read" "" "$TOKEN")
    HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
    if [ "$HTTP_CODE" = "200" ]; then
        print_test "PUT /api/notify/{id}/read" "PASS"
    else
        print_test "PUT /api/notify/{id}/read" "FAIL" "HTTP $HTTP_CODE"
    fi
else
    echo -e "${YELLOW}⚠${NC} PUT /api/notify/{id}/read (skipped - no notifications)"
fi

# Test 11: Unlike Feed
RESPONSE=$(api_call DELETE "/feed/${FEED_ID}/like" "" "$TOKEN")
HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
if [ "$HTTP_CODE" = "200" ]; then
    print_test "DELETE /api/feed/{id}/like" "PASS"
else
    print_test "DELETE /api/feed/{id}/like" "FAIL" "HTTP $HTTP_CODE"
fi

# Test 12-15: Top Statistics (public endpoints)
echo ""
echo "Testing top statistics endpoints..."
for endpoint in "/top/users-liked" "/top/feeds-commented" "/top/feeds-viewed" "/top/feeds-liked"; do
    RESPONSE=$(api_call GET "${endpoint}?page=1&limit=10" "" "")
    HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
    if [ "$HTTP_CODE" = "200" ]; then
        print_test "GET ${endpoint}" "PASS"
    else
        print_test "GET ${endpoint}" "FAIL" "HTTP $HTTP_CODE"
    fi
done

# Summary
echo ""
echo "========================================="
echo "Test Summary"
echo "========================================="
echo -e "${GREEN}Passed: $TESTS_PASSED${NC}"
echo -e "${RED}Failed: $TESTS_FAILED${NC}"
echo "Total: $((TESTS_PASSED + TESTS_FAILED))"
echo ""

if [ $TESTS_FAILED -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed.${NC}"
    exit 1
fi

