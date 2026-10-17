// 全局滚动条（自绘 overlay）：原生滚动条一旦定制就会占布局宽（左右边距不一致的根源），
// 所以彻底隐藏原生条，由本 action 挂一个绝对定位的悬浮拇指——不占布局宽，
// 滚动中浮现、静止 800ms 淡出。亮面容器加 class "sb-light"。
export function scrollFade(node: HTMLElement) {
  node.classList.add("sb-fade");
  if (getComputedStyle(node).position === "static") node.style.position = "relative";
  const thumb = document.createElement("div");
  thumb.className = "sb-thumb";
  node.appendChild(thumb);

  let timer: ReturnType<typeof setTimeout> | undefined;
  const update = () => {
    const { clientHeight, scrollHeight, scrollTop } = node;
    if (scrollHeight <= clientHeight + 1) {
      thumb.style.display = "none";
      return;
    }
    const pad = 4;
    const track = clientHeight - pad * 2;
    const h = Math.max(32, Math.round((track * clientHeight) / scrollHeight));
    const max = scrollHeight - clientHeight;
    const y = pad + (track - h) * (max > 0 ? scrollTop / max : 0);
    // 拇指是滚动容器的绝对定位子元素，会随内容滚走——top 用内容坐标抵消
    thumb.style.height = h + "px";
    thumb.style.top = scrollTop + y + "px";
  };
  const onScroll = () => {
    node.classList.add("sb-active");
    update();
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => node.classList.remove("sb-active"), 800);
  };
  node.addEventListener("scroll", onScroll, { passive: true });
  const ro = new ResizeObserver(update);
  ro.observe(node);
  // 数据装载改变 scrollHeight（骨架 → 真实卡片），内容变化也要重算
  const mo = new MutationObserver(update);
  mo.observe(node, { childList: true, subtree: true });
  update();

  return {
    destroy() {
      node.removeEventListener("scroll", onScroll);
      ro.disconnect();
      mo.disconnect();
      if (timer) clearTimeout(timer);
      thumb.remove();
    },
  };
}
