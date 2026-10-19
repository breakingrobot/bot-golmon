// Thème choisi (clair, sombre ou système) appliqué avant le rendu, pour éviter un flash.
;(function () {
  var choice = 'system'
  try {
    choice = localStorage.getItem('theme') || 'system'
  } catch (e) {}
  var dark = choice === 'dark' || (choice !== 'light' && window.matchMedia('(prefers-color-scheme: dark)').matches)
  document.documentElement.classList.toggle('dark', dark)
  document.documentElement.style.colorScheme = dark ? 'dark' : 'light'
})()
