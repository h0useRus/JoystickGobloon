// Joystick Gobloon website: mobile menu, scroll reveals, the word by word tagline and the
// current section in the navigation. No scroll listeners: everything runs on IntersectionObserver.

(() => {
  const root = document.documentElement;
  const toggle = document.querySelector(".menu-toggle");
  const menu = document.getElementById("menu");

  const setMenu = (open) => {
    document.body.classList.toggle("menu-open", open);
    toggle.setAttribute("aria-expanded", String(open));
    toggle.setAttribute("aria-label", open ? "Close menu" : "Open menu");
    menu.setAttribute("aria-hidden", String(!open));
    root.style.overflow = open ? "hidden" : "";
  };
  toggle.addEventListener("click", () => setMenu(!document.body.classList.contains("menu-open")));
  menu.addEventListener("click", (e) => {
    if (e.target.closest("a")) setMenu(false);
  });
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") setMenu(false);
  });

  // The download buttons name the visitor's system.
  const platform = (navigator.userAgentData?.platform || navigator.platform || "").toLowerCase();
  const label = platform.includes("win")
    ? "Download for Windows"
    : platform.includes("linux") && !/android/i.test(navigator.userAgent)
      ? "Download for Linux"
      : null;
  if (label) document.querySelectorAll("[data-download-label]").forEach((el) => (el.textContent = label));

  if (!("IntersectionObserver" in window)) {
    document.querySelectorAll(".reveal").forEach((el) => el.classList.add("in"));
    document.querySelectorAll(".tagline .w").forEach((el) => el.classList.add("on"));
    return;
  }

  // Sections rise into place once.
  const reveal = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          entry.target.classList.add("in");
          reveal.unobserve(entry.target);
        }
      }
    },
    { rootMargin: "0px 0px -10% 0px" },
  );
  document.querySelectorAll(".reveal").forEach((el) => reveal.observe(el));

  // Tagline: each word lights up as it crosses a line 60% down the viewport.
  document.querySelectorAll("[data-reveal-words]").forEach((p) => {
    const words = p.textContent.trim().split(/\s+/);
    p.textContent = "";
    words.forEach((word, i) => {
      const span = document.createElement("span");
      span.className = "w";
      span.textContent = word;
      p.append(span);
      if (i < words.length - 1) p.append(" ");
    });
  });
  const words = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          entry.target.classList.add("on");
          words.unobserve(entry.target);
        }
      }
    },
    { rootMargin: "0px 0px -40% 0px" },
  );
  document.querySelectorAll(".tagline .w").forEach((el) => words.observe(el));

  // The navigation marks the section on screen.
  const links = new Map(
    [...document.querySelectorAll(".nav-links a")].map((a) => [a.getAttribute("href").slice(1), a]),
  );
  const current = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        const link = links.get(entry.target.id);
        if (!link) continue;
        if (entry.isIntersecting) {
          links.forEach((a) => a.removeAttribute("aria-current"));
          link.setAttribute("aria-current", "true");
        } else if (link.hasAttribute("aria-current")) {
          link.removeAttribute("aria-current");
        }
      }
    },
    { rootMargin: "-45% 0px -50% 0px" },
  );
  links.forEach((_, id) => {
    const section = document.getElementById(id);
    if (section) current.observe(section);
  });
})();
