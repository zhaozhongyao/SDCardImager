#!/bin/bash
# 授予 flash-helper 完全磁盘访问权限 (FDA) 并重启 LaunchDaemon
# 用法: bash grant-fda.sh <flash-helper路径>
set -e

HELPER="$1"
DB="/Library/Application Support/com.apple.TCC/TCC.db"
LABEL="com.sdcard.imageflasher.helper"
PLIST="/Library/LaunchDaemons/${LABEL}.plist"

if [ -z "$HELPER" ] || [ ! -x "$HELPER" ]; then
  echo "错误: flash-helper 不存在或不可执行: $HELPER"
  exit 1
fi

echo "==> 授权完全磁盘访问: $HELPER"
sqlite3 "$DB" "INSERT OR REPLACE INTO access(service, client, client_type, auth_value, auth_reason, auth_version, flags, last_modified) VALUES('kTCCServiceSystemPolicyAllFiles', '$HELPER', 1, 2, 1, 1, 0, strftime('%Y-%m-%d %H:%M:%S','now'));"
echo "    done."

echo "==> 重启守护进程"
if [ -f "$PLIST" ]; then
  launchctl bootout "system/$LABEL" 2>/dev/null || true
  sleep 1
  launchctl bootstrap system "$PLIST"
  sleep 2
  launchctl kickstart "system/$LABEL" 2>/dev/null || true
fi

PIDFILE=/tmp/flash-helper.pid
if [ -f "$PIDFILE" ]; then
  rm -f "$PIDFILE"
fi
echo "==> 完成"
