#!/bin/bash
# TinyFish REST API Client — адаптировано для WATERS
# Источник: https://github.com/edxeth/superlight-tinyfish-skill
# Endpoints: Search (GET https://api.search.tinyfish.ai), Fetch (POST https://api.fetch.tinyfish.ai)
# Документация: https://docs.tinyfish.ai/

set -euo pipefail

readonly SEARCH_API_BASE="https://api.search.tinyfish.ai"
readonly FETCH_API_BASE="https://api.fetch.tinyfish.ai"
readonly API_KEYS="${TINYFISH_API_KEY:-}"
readonly KEY_STATE_FILE="${TMPDIR:-/tmp}/.tinyfish-key-idx-${UID:-0}"
readonly KEY_LOCK_FILE="${TMPDIR:-/tmp}/.tinyfish-key-lock"
readonly MAX_RETRIES=3
readonly BASE_DELAY=1

get_key_count() {
    [[ -z "$API_KEYS" ]] && { echo 0; return; }
    IFS=',' read -ra keys <<< "$API_KEYS"
    echo ${#keys[@]}
}

select_next_api_key() {
    [[ -z "$API_KEYS" ]] && return

    IFS=',' read -ra keys <<< "$API_KEYS"
    local count=${#keys[@]}

    [[ $count -eq 1 ]] && { echo "${keys[0]}"; return; }

    local idx=0
    (
        flock -w 1 200 2>/dev/null || true
        [[ -f "$KEY_STATE_FILE" ]] && idx=$(cat "$KEY_STATE_FILE" 2>/dev/null || echo 0)
        local next_idx=$(( (idx + 1) % count ))
        local tmp_file="${KEY_STATE_FILE}.tmp.$$"
        echo "$next_idx" > "$tmp_file" && mv "$tmp_file" "$KEY_STATE_FILE"
    ) 200>"$KEY_LOCK_FILE"

    [[ -f "$KEY_STATE_FILE" ]] && idx=$(cat "$KEY_STATE_FILE" 2>/dev/null || echo 0)
    idx=$(( (idx + count - 1) % count ))
    echo "${keys[$idx]}"
}

urlencode() {
    local string="$1"
    python3 -c "import urllib.parse; print(urllib.parse.quote('''$string''', safe=''))" 2>/dev/null \
        || printf '%s' "$string" | jq -sRr @uri 2>/dev/null \
        || printf '%s' "$string"
}

do_request() {
    local method="$1"
    local url="$2"
    local data="${3:-}"
    local timeout="${4:-30}"
    local key_count attempts_per_round total_attempts max_attempts round
    key_count=$(get_key_count)
    attempts_per_round=$((key_count > 1 ? key_count : 1))
    total_attempts=0
    max_attempts=$((attempts_per_round * MAX_RETRIES))
    round=0

    while [[ $total_attempts -lt $max_attempts ]]; do
        local api_key http_code response
        api_key=$(select_next_api_key)

        if [[ -z "$api_key" ]]; then
            echo "ERROR: TINYFISH_API_KEY не установлен. Получить: https://agent.tinyfish.ai/api-keys" >&2
            exit 1
        fi

        local -a curl_args=(-sS -w "%{http_code}" --max-time "$timeout")
        curl_args+=(-H "X-API-Key: $api_key")

        if [[ "$method" == "POST" ]]; then
            curl_args+=(-H "Content-Type: application/json")
            curl_args+=(-X POST -d "$data")
        fi

        response=$(curl "${curl_args[@]}" "$url" 2>/dev/null) || true
        http_code="${response: -3}"
        response="${response%???}"

        case "$http_code" in
            200) echo "$response"; return 0 ;;
            429|500|502|503|504|000)
                total_attempts=$((total_attempts + 1))
                if [[ $((total_attempts % attempts_per_round)) -eq 0 ]]; then
                    round=$((round + 1))
                    local delay=$((BASE_DELAY * (2 ** (round - 1))))
                    [[ $delay -gt 16 ]] && delay=16
                    sleep "$delay"
                fi
                ;;
            401)
                echo "ERROR: Неверный API-ключ. Проверьте TINYFISH_API_KEY." >&2
                return 1
                ;;
            400)
                echo "ERROR: Неверный запрос - $response" >&2
                return 1
                ;;
            403)
                echo "ERROR: Доступ запрещён - $response" >&2
                return 1
                ;;
            402)
                echo "ERROR: Требуется оплата - активная подписка." >&2
                return 1
                ;;
            *)
                echo "ERROR: HTTP $http_code - $response" >&2
                return 1
                ;;
        esac
    done

    echo "ERROR: Rate limit на всех ключах после $MAX_RETRIES попыток" >&2
    return 1
}

cmd_search() {
    local query="${1:-}"
    local location="${2:-}"
    local language="${3:-}"
    local page="${4:-}"

    if [[ -z "$query" ]]; then
        echo "Использование: tinyfish.sh search <запрос> [страна] [язык] [страница]"
        echo "Пример: tinyfish.sh search \"web automation tools\" US en"
        exit 1
    fi

    local query_encoded url
    query_encoded=$(urlencode "$query")
    url="${SEARCH_API_BASE}?query=${query_encoded}"

    [[ -n "$location" ]] && url="${url}&location=$(urlencode "$location")"
    [[ -n "$language" ]] && url="${url}&language=$(urlencode "$language")"
    [[ -n "$page" ]] && url="${url}&page=${page}"

    local response
    if ! response=$(do_request "GET" "$url" "" 30); then
        echo "ERROR: Поиск не удался." >&2
        exit 1
    fi

    echo "$response" | jq -r '
        if .results and (.results | length > 0) then
            "\(.results | length) результатов для \"" + (.query // "запрос") + "\":\n" +
            (.results | to_entries | map(
                "\(.key + 1). \(.value.title // "Без заголовка")\n   URL: \(.value.url)\n   \(.value.snippet // "")"
            ) | join("\n\n"))
        elif .error then
            "ERROR: " + (.error.message // (.error | tostring))
        else
            "Результатов не найдено."
        end
    ' 2>/dev/null || echo "$response"
}

cmd_fetch() {
    if [[ $# -eq 0 ]]; then
        echo "Использование: tinyfish.sh fetch <url1> [url2 ...] [--format markdown|html|json] [--links]"
        echo "Пример: tinyfish.sh fetch \"https://example.com\""
        exit 1
    fi

    local urls=()
    local format="markdown"
    local links=false
    local image_links=false

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --format) shift; format="${1:-markdown}" ;;
            --links) links=true ;;
            --image-links) image_links=true ;;
            -*) echo "Неизвестный флаг: $1" >&2; exit 1 ;;
            *) urls+=("$1") ;;
        esac
        shift
    done

    if [[ ${#urls[@]} -eq 0 ]]; then
        echo "ERROR: Требуется хотя бы один URL." >&2
        exit 1
    fi

    if [[ ${#urls[@]} -gt 10 ]]; then
        echo "ERROR: Максимум 10 URL за запрос." >&2
        exit 1
    fi

    local urls_array
    urls_array=$(printf '%s\n' "${urls[@]}" | jq -R . | jq -s .)

    local json_payload
    json_payload=$(jq -n \
        --argjson urls "$urls_array" \
        --arg format "$format" \
        --argjson links "$links" \
        --argjson image_links "$image_links" \
        '{
            urls: $urls,
            format: $format,
            links: $links,
            image_links: $image_links
        }'
    )

    local response
    if ! response=$(do_request "POST" "$FETCH_API_BASE" "$json_payload" 150); then
        echo "ERROR: Загрузка не удалась." >&2
        exit 1
    fi

    echo "$response" | jq -r '
        if .results and (.results | length > 0) then
            (.results | to_entries | map(
                "## \(.value.title // "Page Content")\nURL: \(.value.url // "unknown")\n" +
                (if .value.final_url and .value.final_url != .value.url then "Final URL: \(.value.final_url)\n" else "" end) +
                (if .value.description then "Description: \(.value.description)\n" else "" end) +
                (if .value.language then "Language: \(.value.language)\n" else "" end) +
                (if (.value.text | type) == "string" then
                    "\n\(.value.text)"
                elif (.value.text | type) == "object" then
                    "\n\(.value.text | tojson)"
                else
                    ""
                end) +
                (if .value.links and (.value.links | length) > 0 then "\n\nLinks (\(.value.links | length)):\n" + (.value.links[:10] | join("\n")) else "" end) +
                (if .value.image_links then "\n\nImages (\(.value.image_links | length)):\n" + (.value.image_links[:10] | join("\n")) else "" end)
            ) | join("\n\n---\n\n"))
        else ""
        end
    ' 2>/dev/null

    echo "$response" | jq -r '
        if .errors and (.errors | length > 0) then
            "\n## Errors\n" + (.errors | map("\(.url): \(.error)") | join("\n"))
        else ""
        end
    ' 2>/dev/null

    if [[ "$links" == "true" ]]; then
        local has_html_links
        has_html_links=$(echo "$response" | jq -r '[.results[] | select(.links != null and (.links | length) > 0)] | length')
        if [[ "$has_html_links" == "0" ]]; then
            local md_links
            md_links=$(echo "$response" | jq -r '.results[] | select(.text != null and (.text | type) == "string") | .text' 2>/dev/null | grep -oP 'https?://[^\s)\]>"]+' | sort -u || true)
            if [[ -n "$md_links" ]]; then
                local count
                count=$(echo "$md_links" | wc -l)
                echo ""
                echo "## Extracted URLs (from content)"
                echo "$md_links" | head -50
                if [[ "$count" -gt 50 ]]; then
                    echo "... и ещё $((count - 50))"
                fi
            fi
        fi
    fi
}

cmd_validate() {
    if [[ -z "$API_KEYS" ]]; then
        echo "ERROR: TINYFISH_API_KEY не установлен." >&2
        exit 1
    fi

    IFS=',' read -ra keys <<< "$API_KEYS"
    local total=${#keys[@]}
    local passed=0
    local failed=0

    echo "Проверка $total TinyFish API ключей..."

    for i in "${!keys[@]}"; do
        local key="${keys[$i]}"
        local masked="${key:0:12}...${key: -4}"
        echo -n "Ключ $((i + 1)) ($masked): "

        local response
        response=$(TINYFISH_API_KEY="$key" do_request "GET" "${SEARCH_API_BASE}?query=test&limit=1" "" 10 2>/dev/null) || true
        local exit_code=$?

        if [[ $exit_code -eq 0 ]] && [[ -n "$response" ]]; then
            echo "OK"
            passed=$((passed + 1))
        else
            local http_code
            http_code=$(TINYFISH_API_KEY="$key" curl -sS -w "%{http_code}" --max-time 10 \
                -H "X-API-Key: $key" \
                "${SEARCH_API_BASE}?query=test&limit=1" 2>/dev/null || true)
            http_code="${http_code: -3}"
            case "$http_code" in
                401) echo "НЕДЕЙСТВИТЕЛЕН (401)" ;;
                402) echo "НЕТ КРЕДИТОВ (402)" ;;
                403) echo "ДОСТУП ЗАПРЕЩЁН (403)" ;;
                429) echo "ЛИМИТ (429)" ;;
                *) echo "ОШИБКА (HTTP $http_code)" ;;
            esac
            failed=$((failed + 1))
        fi
    done

    echo ""
    if [[ $failed -eq 0 ]]; then
        echo "Результат: все $total ключей валидны."
    else
        echo "Результат: $passed/$total валидны, $failed невалидны."
    fi
}

case "${1:-}" in
    search)
        shift
        cmd_search "$@"
        ;;
    fetch)
        shift
        cmd_fetch "$@"
        ;;
    validate)
        shift
        cmd_validate "$@"
        ;;
    -h|--help|help)
        cat <<'EOF'
TinyFish Web Search & Scraping — WATERS Edition

Использование:
  tinyfish.sh search <запрос> [страна] [язык] [страница]    Поиск в вебе
  tinyfish.sh fetch <url1> [url2 ...] [флаги]               Извлечение контента
  tinyfish.sh validate                                       Проверка API-ключей

Флаги (для fetch):
  --format <format>    Формат: markdown (по умолч.), html, json
  --links              Включить ссылки в вывод
  --image-links        Включить ссылки на изображения

Переменные окружения:
  TINYFISH_API_KEY    API-ключ(и) TinyFish (обязательно)
                      Поддерживает несколько ключей через запятую
EOF
        ;;
    *)
        echo "Использование: tinyfish.sh {search|fetch|validate} [args...]"
        echo "Запустите 'tinyfish.sh --help' для примеров"
        exit 1
        ;;
esac
