# Tester

Kjør `node --test tests/core.cjs`. Testene bruker bare Node og syntetiske data.
Ingen personopplysninger fra eierens prosjekt følger med.

For nettlesertest: installer Playwright med `npm install`, og kjør `npm test`.
Windows bruker Edge. På andre plattformer kan Chromium installeres med
`npx playwright install chromium`. `CVSTUDIO_BROWSER_CHANNEL` kan angi en annen
installert nettleserkanal. Filene eksporteres til en midlertidig testmappe.

Testene åpner pakken direkte via `file://` og avviser bakgrunnsforespørsler til
nettet. De kontrollerer også at import og redigering ugyldiggjør faktagodkjenning.
Dokumentlayout i mottakerens Word-program må kontrolleres separat.
