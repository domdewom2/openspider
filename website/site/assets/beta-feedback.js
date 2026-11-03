// Optional enhancement of the beta feedback form. The form submits and shows errors without it.
(function () {
  var form = document.getElementById('feedback-form');
  if (!form || !window.URLSearchParams) return;
  // Prefill the version from ?version= (the app's Help menu link); never overwrite a typed value.
  var version = new URLSearchParams(window.location.search).get('version');
  var field = form.elements.namedItem('version');
  if (version && field && !field.value) field.value = version.replace(/[\u0000-\u001f\u007f]/g, '').trim().slice(0, 64);
  // Move keyboard and screen reader focus to an error message shown via #feedback-error-*.
  if (/^#feedback-error-[a-z_]+$/.test(window.location.hash)) {
    var error = document.getElementById(window.location.hash.slice(1));
    if (error) error.focus();
  }
})();
