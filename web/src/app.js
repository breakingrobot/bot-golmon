// Dates dans le fuseau horaire du lecteur (le serveur les rend en UTC), au
// style demandé par Discord (<t:…:R> relatif, <t:…:D> date longue, etc.).
const FORMATS = {
  t: { timeStyle: "short" },
  T: { timeStyle: "medium" },
  d: { dateStyle: "short" },
  D: { dateStyle: "long" },
  f: { dateStyle: "long", timeStyle: "short" },
  F: { dateStyle: "full", timeStyle: "short" },
};

const UNITS = [
  ["year", 31536000],
  ["month", 2592000],
  ["week", 604800],
  ["day", 86400],
  ["hour", 3600],
  ["minute", 60],
  ["second", 1],
];

function relative(date) {
  const seconds = (date.getTime() - Date.now()) / 1000;
  const [unit, size] = UNITS.find(([, size]) => Math.abs(seconds) >= size) ?? UNITS.at(-1);
  return new Intl.RelativeTimeFormat(undefined, { numeric: "auto" }).format(Math.round(seconds / size), unit);
}

document.querySelectorAll("time[data-local]").forEach((element) => {
  const date = new Date(element.getAttribute("datetime"));
  if (Number.isNaN(date.getTime())) return;
  const style = element.dataset.format;
  if (style === "R") {
    element.textContent = relative(date);
  } else {
    const options = element.hasAttribute("data-time-only")
      ? { hour: "2-digit", minute: "2-digit" }
      : FORMATS[style] ?? FORMATS.f;
    element.textContent = new Intl.DateTimeFormat(undefined, options).format(date);
  }
  element.title = date.toLocaleString();
});

// Spoilers : un clic (ou Entrée) les révèle.
document.querySelectorAll(".spoiler").forEach((spoiler) => {
  const reveal = () => spoiler.classList.add("revealed");
  spoiler.addEventListener("click", reveal);
  spoiler.addEventListener("keydown", (event) => {
    if (event.key === "Enter" || event.key === " ") reveal();
  });
});
