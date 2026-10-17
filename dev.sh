#!/usr/bin/env bash
# MLCanvas 开发脚本：统一前端/Rust 构建顺序、启动与冒烟验证。
#
# 为什么需要它（历史事故，详见 AGENTS.md 认知修正）：
#   1. plain `cargo build` 不带 custom-protocol feature 时，tauri codegen 不内嵌
#      前端资产，产出"等 dev server"的空壳二进制 → 启动白屏 asset not found。
#   2. 改前端后只跑 npm run build 不重编 Rust，跑的还是旧内嵌产物。
#   3. git stash 切换源码后忘记重跑对应构建，二进制/产物处于中间态。
#   4. 资产嵌入的重编译触发不可靠，必须用 strings 验证内嵌结果。
# 用法：
#   ./dev.sh build   # 前端 + Rust 构建 + 资产内嵌校验
#   ./dev.sh run     # build + 重启应用 + 桥连通检查
#   ./dev.sh smoke   # 对运行中的应用做冒烟：桥 ping、页面 URL、主页截图
#   ./dev.sh clean   # 清理 mlcanvas 编译产物后完整重建
#   ./dev.sh vendor  # 拉取 OpenDesign skills 参考库到 resources/（幂等，按 pin 判断）
# 说明：MCP 服务已内嵌进应用进程（mcp_http.rs，:9916/mcp），随应用启停，无需单独管理。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
APP="$ROOT/app"
TAURI="$APP/src-tauri"
BIN="$TAURI/target/debug/mlcanvas"
BRIDGE_HOST="127.0.0.1"; BRIDGE_PORT=9915
LOG="/tmp/mlcanvas.log"

step() { printf '\n\033[1;36m== %s\033[0m\n' "$*"; }
ok()   { printf '\033[1;32m✅ %s\033[0m\n' "$*"; }
die()  { printf '\033[1;31m❌ %s\033[0m\n' "$*" >&2; exit 1; }

build_front() {
  step "构建前端 (vite)"
  (cd "$APP" && npm run build --silent)
  [ -f "$APP/dist/index.html" ] || die "dist/index.html 不存在"
  ok "dist 就绪"
}

build_rust() {
  step "构建 Rust (custom-protocol：内嵌前端资产，缺它必白屏)"
  (cd "$TAURI" && cargo build --features custom-protocol)
  [ -x "$BIN" ] || die "二进制不存在: $BIN"
}

verify_embed() {
  step "校验前端资产已内嵌"
  # 注意不能用 grep -q：pipefail 下 -q 提前关闭管道会给 strings 传 SIGPIPE（退出 141）判负
  local n
  n=$(LC_ALL=C strings "$BIN" | LC_ALL=C grep -c 'assets/index-.*\.js' || true)
  if [ "${n:-0}" -gt 0 ]; then
    ok "资产已内嵌"
    return
  fi
  die "二进制内没有前端资产（白屏预警）。
   修复：./dev.sh clean   # 清理后完整重建"
}

launch() {
  step "重启应用"
  pkill -f "target/debug/mlcanvas" 2>/dev/null || true
  sleep 1
  # 后台直启，不经 `open`/默认终端（open 裸二进制会拉起 Terminal 窗口）
  nohup "$BIN" >"$LOG" 2>&1 &
  for _ in $(seq 1 10); do
    sleep 1
    if nc -z "$BRIDGE_HOST" "$BRIDGE_PORT" 2>/dev/null; then ok "桥已监听 ($BRIDGE_HOST:$BRIDGE_PORT)"; return; fi
  done
  die "桥 10 秒未监听，应用可能启动失败（看 stderr：nohup 启动可留日志）"
}

smoke() {
  step "冒烟验证（桥 / 页面导航 / 主页截图）"
  BRIDGE_PORT="$BRIDGE_PORT" BRIDGE_HOST="$BRIDGE_HOST" node -e '
    const net = require("node:net");
    function call(tool, args = {}) {
      return new Promise((resolve, reject) => {
        const id = Date.now() + "-" + Math.random().toString(36).slice(2);
        const s = net.connect(+process.env.BRIDGE_PORT, process.env.BRIDGE_HOST);
        s.setEncoding("utf8"); let buf = "";
        const t = setTimeout(() => { s.destroy(); reject(new Error("超时")); }, 20000);
        s.on("connect", () => s.write(JSON.stringify({ id, tool, args }) + "\n"));
        s.on("data", (c) => { buf += c; const i = buf.indexOf("\n"); if (i === -1) return;
          clearTimeout(t); const m = JSON.parse(buf.slice(0, i)); s.end();
          m.ok ? resolve(m.result) : reject(new Error(m.error)); });
        s.on("error", (e) => { clearTimeout(t); reject(new Error(e.code)); });
      });
    }
    (async () => {
      console.log("ping:", JSON.stringify(await call("ping")));
      let label = "main";
      try { await call("debug_window_url", { label }); }
      catch { label = "design"; }
      const u = await call("debug_window_url", { label });
      console.log(label, "url:", u.url);
      if (!/^tauri:\/\/localhost/.test(u.url)) {
        console.log("❌ 页面未完成导航（白屏状态）"); process.exit(1);
      }
      const shot = await call("debug_shell_screenshot", { label });
      console.log("截图:", shot.path, "(" + shot.bytes + " bytes)");
    })().catch((e) => { console.log("❌ 冒烟失败:", e.message); process.exit(1); });
  '

  step "画板资源服务回归（9917 / 防穿越 / 运行时注入）"
  local files_base="http://127.0.0.1:9917/files"
  local proj enc_board board_file
  board_file="$(ls "$HOME/YuCanvas/示例项目/artboards/"*.html 2>/dev/null | head -1 | xargs basename)"
  if [ -n "$board_file" ] && curl -s -o /dev/null --max-time 3 http://127.0.0.1:9917/__rt.js; then
    proj="$(python3 -c "import urllib.parse;print(urllib.parse.quote('示例项目'))")"
    enc_board="$(python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1]))" "$board_file")"
    # 画板可取且注入了运行时
    if ! curl -sf "$files_base/$proj/artboards/$enc_board" | head -c 4000 | grep -q "__rt.js"; then
      die "画板未注入运行时脚本"
    fi
    # tokens.css 可取且 MIME 正确
    [ "$(curl -s -o /dev/null -w '%{http_code}:%{content_type}' "$files_base/$proj/tokens.css" | cut -d';' -f1)" = "200:text/css" ] \
      || die "tokens.css 加载失败或 MIME 错误"
    # 目录穿越必须被拦（原始与编码两种写法）
    [ "$(curl -s --path-as-is -o /dev/null -w '%{http_code}' "$files_base/$proj/../.index.json")" = "403" ] \
      || die "目录穿越未拦截（原始）"
    [ "$(curl -s --path-as-is -o /dev/null -w '%{http_code}' "$files_base/$proj/%2e%2e/.index.json")" = "403" ] \
      || die "目录穿越未拦截（编码）"
    ok "资源服务通过（运行时注入 / tokens.css / 防穿越）"
  else
    echo "  （跳过：资源服务未启动或示例项目无画板）"
  fi

  ok "冒烟通过"
}

OD_REPO="https://github.com/nexu-io/open-design"
OD_COMMIT="53231d40b778d88eba23f35547bf99485d3ae9fc"
RES="$TAURI/resources/opendesign"

vendor() {
  step "vendor OpenDesign skills + design-systems @ ${OD_COMMIT:0:9}"
  if [ -d "$RES/skills" ] && [ -f "$RES/design-systems/index.json" ] && [ "$(cat "$RES/commit" 2>/dev/null)" = "$OD_COMMIT" ]; then
    ok "已是 pinned 版本，跳过"
    return
  fi
  rm -rf "$RES"
  mkdir -p "$RES"
  local tmp; tmp="$(mktemp -d "${TMPDIR:-/tmp}/mlc-opendesign-XXXXXX")"
  # 只取 pinned commit 的 skills/ 与 design-systems/：blob:none + sparse，
  # 不落盘仓库其余内容（CI 与本地一致，可复现）

  (
    cd "$tmp"
    git init --quiet
    git remote add origin "$OD_REPO"
    # 大仓 + 代理环境下 HTTP/2 易断，强制 1.1
    git -c http.version=HTTP/1.1 fetch --quiet --depth 1 --filter=blob:none origin "$OD_COMMIT"
  ) || die "拉取 OpenDesign 失败（检查网络）"
  (cd "$tmp" && git sparse-checkout init --cone && git sparse-checkout set skills design-systems && git checkout --quiet FETCH_HEAD) \
    || die "检出 skills / design-systems 失败"
  mv "$tmp/skills" "$RES/skills"
  mv "$tmp/design-systems" "$RES/design-systems"
  rm -rf "$tmp"
  echo "$OD_COMMIT" > "$RES/commit"
  ok "skills: $(ls "$RES/skills" | wc -l | tr -d ' ') 个"
  # design-systems 剪枝：只留 UI 与 Agent 要用的文件，去掉审计/图片/衍生格式
  find "$RES/design-systems" -type d -name source -exec rm -rf {} + 2>/dev/null || true
  find "$RES/design-systems" \( -name "*.png" -o -name "*.jpg" -o -name "*.jpeg" -o -name "*.webp" -o -name "*.mp4" -o -name "design-tokens.json" -o -name "tailwind-v4.css" \) -delete 2>/dev/null || true
  ok "design-systems: $(ls "$RES/design-systems" | wc -l | tr -d ' ') 包, $(du -sh "$RES/design-systems" | cut -f1)"
  # vendor 期生成索引：运行期只读这一个文件
  node "$ROOT/scripts/gen-ds-index.mjs" "$RES/design-systems" "$RES/design-systems/index.json"
}

case "${1:-run}" in
  build) build_front; build_rust; verify_embed ;;
  run)   build_front; build_rust; verify_embed; launch ;;
  smoke) smoke ;;
  clean) step "清理并完整重建"; (cd "$TAURI" && cargo clean -p mlcanvas); build_front; build_rust; verify_embed ;;
  vendor) vendor ;;
  *) echo "用法: $0 {build|run|smoke|clean|vendor}"; exit 1 ;;
esac
