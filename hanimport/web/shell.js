(function () {
  "use strict";

  var NAV_ITEMS = [
    { key: "hub", href: "/", label: "概览" },
    { key: "unpack", href: "/unpack", label: "解包" },
    { key: "roster", href: "/roster", label: "角色库" },
    { key: "skins", href: "/skins", label: "皮肤" },
  ];

  function navMarkup(active) {
    return NAV_ITEMS.map(function (item) {
      var cls = item.key === active ? ' class="is-active"' : "";
      var current = item.key === active ? ' aria-current="page"' : "";
      return (
        '<a href="' +
        item.href +
        '" data-nav="' +
        item.key +
        '"' +
        cls +
        current +
        ">" +
        item.label +
        "</a>"
      );
    }).join("");
  }

  window.HanShell = {
    mount: async function mount(opts) {
      var active = (opts && opts.active) || "hub";
      if (document.querySelector(".app-shell")) {
        return;
      }

      var root = document.getElementById("shell-root") || document.body;
      var header = document.createElement("header");
      header.className = "app-shell";
      header.innerHTML =
        '<div class="app-shell-inner">' +
        '<a class="app-brand" href="/">小寒导入器</a>' +
        '<nav class="app-nav" aria-label="主导航">' +
        navMarkup(active) +
        "</nav>" +
        '<a class="status-dot" href="/" aria-label="环境状态"></a>' +
        "</div>";

      root.insertBefore(header, root.firstChild);

      try {
        var res = await fetch("/api/status");
        if (!res.ok) throw new Error("status " + res.status);
        var data = await res.json();
        var dot = header.querySelector(".status-dot");
        if (!dot) return;
        var bad = !data.unitypy;
        dot.classList.add(bad ? "warn" : "ok");
        dot.title = bad ? "环境需注意（见概览）" : "环境正常";
      } catch (_err) {
        var fallback = header.querySelector(".status-dot");
        if (fallback) {
          fallback.classList.add("err");
          fallback.title = "无法读取环境状态";
        }
      }
    },
  };
})();
