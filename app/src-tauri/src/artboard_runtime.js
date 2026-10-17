/* 聿Canvas 画板运行时 —— 由资源服务注入每个画板（服务时注入，不进用户文件）。
   职责：① 拦截导航（画板是静态设计图，跳转违反它在画布中的定位）
        ② 样式失败可见性（外链样式没加载成就打标 + 通知壳，白屏不再无声）
        ③ 错误与交互经 postMessage 上报（V2 元素审查/标注的桥就长在这条通道上）。 */
(function () {
  "use strict";

  function report(payload) {
    try {
      payload.source = "yu-artboard";
      parent.postMessage(payload, "*");
    } catch (_) {
      /* 无父窗口（隐藏窗口直载）时静默 */
    }
  }

  // ① 导航拦截：链接点击、表单提交、window.open 一律就地拦下
  function blockNavigation() {
    document.addEventListener(
      "click",
      function (e) {
        var el = e.target;
        var a = el && el.closest ? el.closest("a[href]") : null;
        if (!a) return;
        e.preventDefault();
        e.stopPropagation();
        report({ type: "link", href: a.getAttribute("href") });
      },
      true
    );
    document.addEventListener(
      "submit",
      function (e) {
        e.preventDefault();
      },
      true
    );
    window.open = function () {
      report({ type: "link", href: "(window.open)" });
      return null;
    };
  }

  // ② 样式失败可见性：页面声明了外链样式但没加载进来 → 打标 + 上报
  function checkStyles() {
    var links = document.querySelectorAll('link[rel="stylesheet"][href]');
    if (!links.length) return;
    var loaded = {};
    for (var i = 0; i < document.styleSheets.length; i++) {
      var href = document.styleSheets[i].href;
      if (href) loaded[href.split("?")[0]] = true;
    }
    var missing = 0;
    for (var j = 0; j < links.length; j++) {
      var abs;
      try {
        abs = new URL(links[j].getAttribute("href"), location.href).href.split("?")[0];
      } catch (_) {
        continue;
      }
      if (!loaded[abs]) missing++;
    }
    if (missing) {
      document.documentElement.setAttribute("data-style-missing", String(missing));
      report({ type: "style-missing", count: missing });
    }
  }

  // ③ 错误上报
  window.addEventListener("error", function (e) {
    report({ type: "error", message: String((e && e.message) || "unknown") });
  });

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", blockNavigation);
  } else {
    blockNavigation();
  }
  if (document.readyState === "complete") {
    checkStyles();
  } else {
    window.addEventListener("load", checkStyles);
  }
})();
