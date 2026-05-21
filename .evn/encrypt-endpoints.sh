#!/usr/bin/env bash
set -euo pipefail

EVN_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${EVN_DIR}/.." && pwd)"

ENC_FILE="${EVN_DIR}/endpoints.enc"
TOKEN_FILE="${EVN_DIR}/.gitee-token"

GITEE_OWNER="tuolingshe"
GITEE_REPO="tuoling-shipinhao"
GITEE_BRANCH="master"
GITEE_PATH="endpoints.enc"

bold() { printf '\033[1m%s\033[0m\n' "$1"; }
warn() { printf '\033[33m[警告]\033[0m %s\n' "$1"; }
fail() { printf '\033[31m[错误]\033[0m %s\n' "$1" >&2; exit 1; }
ok()   { printf '\033[32m[OK]\033[0m %s\n' "$1"; }

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "缺少依赖命令：${1}"
}
require_cmd curl
require_cmd jq
require_cmd base64
require_cmd cargo

[[ -f "${TOKEN_FILE}" ]] || fail "缺少 ${TOKEN_FILE}（请把 Gitee 私人访问令牌写入该文件）"
GITEE_TOKEN="$(tr -d ' \t\r\n' < "${TOKEN_FILE}")"
[[ -n "${GITEE_TOKEN}" ]] || fail "${TOKEN_FILE} 内容为空"

bold "步骤 1/3：加密 endpoints.json → endpoints.enc"
(cd "${REPO_ROOT}" && cargo run -p xtask -- encrypt-endpoints)
[[ -f "${ENC_FILE}" ]] || fail "加密产物 ${ENC_FILE} 不存在"
ok "加密完成（$(wc -c < "${ENC_FILE}" | tr -d ' ') 字节）"

gitee_api_url() {
  printf 'https://gitee.com/api/v5/repos/%s/%s/contents/%s' "${GITEE_OWNER}" "${GITEE_REPO}" "$1"
}

bold "步骤 2/3：推送至 Gitee"
API_URL="$(gitee_api_url "${GITEE_PATH}")"
GET_RESP="$(curl -sS -G "${API_URL}" \
  --data-urlencode "access_token=${GITEE_TOKEN}" \
  --data-urlencode "ref=${GITEE_BRANCH}")"

if echo "${GET_RESP}" | jq -e '.sha' >/dev/null 2>&1; then
  CURRENT_SHA="$(echo "${GET_RESP}" | jq -r '.sha')"
  ok "当前 sha=${CURRENT_SHA}"
else
  CURRENT_SHA=""
  warn "文件不存在，将创建"
fi

CONTENT_B64="$(base64 < "${ENC_FILE}" | tr -d '\n')"
TIMESTAMP="$(date '+%Y-%m-%d %H:%M:%S')"

if [[ -n "${CURRENT_SHA}" ]]; then
  BODY="$(jq -n \
    --arg token   "${GITEE_TOKEN}" \
    --arg msg     "chore(endpoints): refresh (${TIMESTAMP})" \
    --arg branch  "${GITEE_BRANCH}" \
    --arg sha     "${CURRENT_SHA}" \
    --arg content "${CONTENT_B64}" \
    '{access_token:$token, message:$msg, branch:$branch, sha:$sha, content:$content}')"
  RESP="$(curl -sS -X PUT "${API_URL}" \
    -H 'Content-Type: application/json;charset=UTF-8' \
    --data-binary "${BODY}")"
else
  BODY="$(jq -n \
    --arg token   "${GITEE_TOKEN}" \
    --arg msg     "chore(endpoints): create (${TIMESTAMP})" \
    --arg branch  "${GITEE_BRANCH}" \
    --arg content "${CONTENT_B64}" \
    '{access_token:$token, message:$msg, branch:$branch, content:$content}')"
  RESP="$(curl -sS -X POST "${API_URL}" \
    -H 'Content-Type: application/json;charset=UTF-8' \
    --data-binary "${BODY}")"
fi

if ! echo "${RESP}" | jq -e '.content.sha' >/dev/null 2>&1; then
  echo "${RESP}" | jq . >&2 2>/dev/null || echo "${RESP}" >&2
  fail "Gitee 推送失败"
fi
NEW_SHA="$(echo "${RESP}" | jq -r '.content.sha')"
ok "Gitee 已更新 ${GITEE_PATH}，新 sha=${NEW_SHA}"

bold "步骤 3/3：验证远端一致性"
RAW_URL="https://gitee.com/${GITEE_OWNER}/${GITEE_REPO}/raw/${GITEE_BRANCH}/${GITEE_PATH}"
REMOTE="$(curl -sSL --max-time 10 "${RAW_URL}")"
LOCAL="$(< "${ENC_FILE}")"
if [[ "${REMOTE}" == "${LOCAL}" ]]; then
  ok "远端与本地一致"
else
  warn "远端尚未刷新（Gitee CDN 可能有缓存延迟）"
fi

bold "完成"
echo "手动复核：https://gitee.com/${GITEE_OWNER}/${GITEE_REPO}/blob/${GITEE_BRANCH}/${GITEE_PATH}"
