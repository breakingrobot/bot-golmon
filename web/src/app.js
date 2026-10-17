// Affiche les dates des logs dans le fuseau horaire du lecteur (le serveur les rend en UTC).
document.querySelectorAll("time[data-local]").forEach((element) => {
  const date = new Date(element.getAttribute("datetime"));
  if (Number.isNaN(date.getTime())) return;
  const options = element.hasAttribute("data-time-only")
    ? { hour: "2-digit", minute: "2-digit" }
    : { dateStyle: "long", timeStyle: "short" };
  element.textContent = new Intl.DateTimeFormat(undefined, options).format(date);
  element.title = date.toLocaleString();
});
