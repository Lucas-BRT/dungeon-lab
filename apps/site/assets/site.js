// Progressive enhancement only: the site works without this file.
(() => {
  const fine = matchMedia("(hover: hover) and (pointer: fine)").matches;
  const still = matchMedia("(prefers-reduced-motion: reduce)").matches;

  // Lantern: a soft light follows the pointer in the deepest layers.
  if (fine && !still) {
    for (const layer of document.querySelectorAll('[data-layer="rock"], [data-layer="deep"]')) {
      layer.addEventListener("pointermove", (e) => {
        const r = layer.getBoundingClientRect();
        layer.style.setProperty("--lx", `${e.clientX - r.left}px`);
        layer.style.setProperty("--ly", `${e.clientY - r.top}px`);
      });
      layer.addEventListener("pointerleave", () => {
        layer.style.removeProperty("--lx");
        layer.style.removeProperty("--ly");
      });
    }
  }

  // Docs: a copy button on every code block.
  const pt = document.documentElement.lang.startsWith("pt");
  const label = pt ? ["Copiar", "Copiado"] : ["Copy", "Copied"];
  for (const pre of document.querySelectorAll(".docs-article pre")) {
    if (!navigator.clipboard) break;
    const wrap = document.createElement("div");
    wrap.className = "code-block";
    pre.replaceWith(wrap);
    wrap.append(pre);
    const button = document.createElement("button");
    button.type = "button";
    button.className = "copy";
    button.textContent = label[0];
    button.addEventListener("click", async () => {
      try {
        await navigator.clipboard.writeText(pre.innerText);
        button.textContent = label[1];
        setTimeout(() => (button.textContent = label[0]), 1600);
      } catch {}
    });
    wrap.append(button);
  }

  // Docs: the table of contents follows the section being read.
  const links = [...document.querySelectorAll(".docs-toc a")];
  if (links.length && "IntersectionObserver" in window) {
    const byId = new Map(links.map((a) => [decodeURIComponent(a.hash.slice(1)), a]));
    const heads = [...byId.keys()].map((id) => document.getElementById(id)).filter(Boolean);
    const seen = new Set();
    const mark = () => {
      const first = heads.find((h) => seen.has(h));
      if (!first) return;
      for (const a of links) a.removeAttribute("aria-current");
      byId.get(first.id)?.setAttribute("aria-current", "location");
    };
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) (e.isIntersecting ? seen.add(e.target) : seen.delete(e.target));
        mark();
      },
      { rootMargin: "-72px 0px -60% 0px" },
    );
    heads.forEach((h) => io.observe(h));
  }
})();
