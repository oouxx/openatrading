#!/bin/bash
set -e

SERVICE="${1:-all}"

usage() {
  echo "Usage: DEPLOY_HOST=<user@host> ./deploy.sh [gateway|web|all]"
  echo "  gateway     Build & deploy gateway container only"
  echo "  web         Build & deploy frontend container only"
  echo "  all         Build & deploy everything (default)"
  echo ""
  echo "环境变量:"
  echo "  DEPLOY_HOST        目标主机 user@host（必填，也可写在 .env 里）"
  echo "  DEPLOY_REMOTE_DIR  远端目录（默认 /root/openatrading）"
  echo "  DEPLOY_REPO_URL    远端要 clone 的仓库（默认读本地 git remote）"
  echo "  DEPLOY_BRANCH      部署分支（默认 main）"
  exit 1
}

case "$SERVICE" in
  gateway|web|all) ;;
  *) usage ;;
  --help|-h) usage ;;
esac

if [ ! -f ".env" ]; then
  echo "ERROR: .env file not found."
  exit 1
fi

# 先加载 .env（GITHUB_TOKEN、DEPLOY_HOST 等都住这里），再解析下面的部署参数。
set -a
source .env
set +a

# 部署目标从环境变量读（不写死在仓库里），因此可以放在 ~/.ssh/config 或 .env。
SERVER="${DEPLOY_HOST:?DEPLOY_HOST not set — 在 .env 里写 DEPLOY_HOST=<user@host>，不要把它提交进仓库}"
REMOTE_DIR="${DEPLOY_REMOTE_DIR:-/root/openatrading}"
SSH_OPTS="-o StrictHostKeyChecking=accept-new"

# Default deploy branch is main; override with DEPLOY_BRANCH=<branch>.
BRANCH="${DEPLOY_BRANCH:-main}"

echo "=== Deploying: $SERVICE → $SERVER (branch: $BRANCH) ==="

REPO_URL="${DEPLOY_REPO_URL:-$(git config --get remote.origin.url 2>/dev/null || echo 'git@github.com:oouxx/openatrading.git')}"

clone_or_pull() {
  echo ""
  echo "=== [git] Fetching latest code from GitHub ==="
  ssh $SSH_OPTS "$SERVER" "
    if [ -d $REMOTE_DIR/.git ]; then
      cd $REMOTE_DIR && git fetch origin && git reset --hard origin/$BRANCH
      echo '✅ git pull done (branch: $BRANCH)'
    elif [ -d $REMOTE_DIR ]; then
      rm -rf $REMOTE_DIR
      git clone -b $BRANCH $REPO_URL $REMOTE_DIR
      echo '✅ old dir removed, git clone done'
    else
      git clone -b $BRANCH $REPO_URL $REMOTE_DIR
      echo '✅ git clone done'
    fi
  "
  # Upload .env separately (not in git); also mirror to ~/.atrading for host-side tooling.
  echo "=== [env] Uploading .env ==="
  scp $SSH_OPTS .env "$SERVER:$REMOTE_DIR/.env"
  ssh $SSH_OPTS "$SERVER" "mkdir -p /root/.atrading && cp $REMOTE_DIR/.env /root/.atrading/.env"
  echo '✅ .env uploaded (and copied to /root/.atrading/)'
}


sync_agent_config() {
  echo ""
  echo "=== [agent] Syncing pi-rs agent config → /root/.atrading/agent ==="
  ssh $SSH_OPTS "$SERVER" "mkdir -p /root/.atrading/agent/skills"

  # settings.json — model / provider config
  # if [ -f .atrading/agent/settings.json ]; then
  #   rsync -az -e "ssh $SSH_OPTS" .atrading/agent/settings.json "$SERVER:/root/.atrading/agent/settings.json"
  #   echo '✅ settings.json synced'
  # else
  #   echo '⚠ .atrading/agent/settings.json not found locally, skipped'
  # fi

  # AGENTS.md — agent instructions
  if [ -f .atrading/agent/AGENTS.md ]; then
    rsync -az -e "ssh $SSH_OPTS" .atrading/agent/AGENTS.md "$SERVER:/root/.atrading/agent/AGENTS.md"
    echo '✅ AGENTS.md synced'
  else
    echo '⚠ .atrading/agent/AGENTS.md not found locally, skipped'
  fi

  # models.json — optional model registry override
  if [ -f .atrading/agent/models.json ]; then
    rsync -az -e "ssh $SSH_OPTS" .atrading/agent/models.json "$SERVER:/root/.atrading/agent/models.json"
    echo '✅ models.json synced'
  fi

  # skills/ — recursive mirror (--delete prunes server-side skills removed locally)
  if [ -d .atrading/agent/skills ]; then
    rsync -az --delete -e "ssh $SSH_OPTS" .atrading/agent/skills/ "$SERVER:/root/.atrading/agent/skills/"
    echo '✅ skills/ synced'
  else
    echo '⚠ .atrading/agent/skills/ not found locally, skipped'
  fi
}

deploy_compose() {
  echo ""
  echo "=== [compose] Building & starting: $SERVICE ==="
  case "$SERVICE" in
    gateway) ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose up -d --build gateway" ;;
    web)     ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose up -d --build frontend" ;;
    all)     ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose up -d --build" ;;
  esac
  echo "✅ compose done ($SERVICE)"
}

clone_or_pull
sync_agent_config
deploy_compose

echo ""
echo "=== Deploy complete ($SERVICE, branch $BRANCH) ==="
