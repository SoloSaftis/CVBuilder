# CVBuilder Familie

Pakk ut hele ZIP-filen og åpne `index.html` i Edge eller Chrome. Ingen innlogging,
server, AI-abonnement eller installasjon trengs for skjema, interessevurdering,
Word-eksport og utskrift til PDF. Ikke åpne HTML direkte inne i ZIP-filen.

## Bruk

1. Fyll ut kontakt, profil og stilling. Feltene har faste navn og formater.
2. Legg til jobber, utdanning, kompetanse og språk. Bekreft egne opplysninger.
3. Legg inn hele interessebanken. Skill mellom aktivitet, tidligere aktivitet og
   interesse. Legg bare inn offentlige arbeidsplassopplysninger du har kontrollert.
4. Se forslagene og begrunnelsene. Velg høyst tre interesser og høyst 35 ord.
   Utvalget kan være tomt. Faglig erfaring har forrang over interesser.
5. Legg eventuelt til portrett. Det lages en liten lokal eksportkopi; originalfilen
   endres ikke. Ingen profilbilder følger med i denne generelle pakken.
6. Kontroller opplysningene og kryss av for faktagodkjenning. Word/PDF aktiveres
   når påkrevde felt er i orden og CV-en passer på maksimalt to A4-sider.
7. Ved PDF: velg A4, ingen topp- og bunntekst og slå på bakgrunnsgrafikk.
   Kontroller alle sider. Kontroller også sidebryting i ditt Word-program.

Lagre utkastet som JSON før du lukker vinduet. Ingen automatisk nettleserlagring.
Prosjektfilen inneholder personopplysninger og eventuelt bilde; del den bevisst.
Ved import må fakta godkjennes på nytt. Språkvalget oversetter overskrifter,
ikke kandidatens tekst. Lagre én kontrollert prosjektfil per språk.

## Strengt skjema

`templates/utkast.json` er den tomme strukturen. `templates/intake.schema.json`
beskriver alle tillatte felt, typer, grenser og enum-verdier. Ukjente felt avvises.
`SKJEMA.md` forklarer formatering og regler. Et gyldig utkast kan være ufullstendig;
en CV-eksport krever i tillegg kontaktdata, tidslinje, bekreftelser og plasskontroll.

`contract.js` er felles validering for nettleser og Node-verktøy. Med Node kan du
kjøre `node tools/validate.cjs prosjekt.json` eller legge til `--ready` for
innholdskontroll før eksport. Ingen ekstra Node-pakker kreves for dette.

## AI og kildearbeid

Les `AI_START_HER.md` og `ai/BUILD_RULES.md`. Ni rolleprompter, oppgavekontrakt,
avhengigheter og godkjenningsregler følger med. De brukes av valgt AI-vert eller
manuelt. Denne pakken starter ikke ni modeller og foretar ingen skjult nettbruk.
Interesseforslagene i skjemaet følger eksplisitte lokale regler.

Offentlig research kan gjennomføres av en tilkoblet AI-vert etter prosedyren i
regelsettet. Ingen gjetting av personlige interesser eller private opplysninger.
Manglende funn skal rapporteres som uavklart. Et hobbytreff er aldri et yrkeskrav.

Valgfritt MCP-oppsett ligger i `ai/mcp/`. Det eksponerer bare arbeidsflyt og
rolleprompter; ikke kandidatdata eller godkjenningsmyndighet. Installasjon og
absolutte prosjektstier er forklart der. Vanlig bruk av byggeren trenger ikke MCP.

Format- og tekstkontroll reduserer risiko for leseproblemer. Den dokumenterer
ingen universell ATS-kompatibilitet eller bestemt intervjusjanse.

## Tester og deling

`node --test tests/core.cjs` tester kontrakter, interessevalg og godkjenningsporter.
Nettlesertesten krever Playwright og Edge/Chromium; se `tests/LES_MEG.md`.
Pakken er laget fra en liste med generelle filer og kontrollert for persondata fra
eierens personlige prosjekt. Bruk den tomme ZIP-en når du deler videre; egne lagrede
utkast og ferdige CV-er blir ikke automatisk anonymisert.
