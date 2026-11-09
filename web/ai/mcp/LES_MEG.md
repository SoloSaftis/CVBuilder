# Valgfri MCP-vert

Vanlig bruk av byggeren trenger ikke MCP. For en AI-vert som støtter MCP:

1. Opprett et eget Python-miljø med `python -m venv .venv`.
2. Installer med `.venv/Scripts/python -m pip install -r ai/mcp/requirements.txt` på Windows, eller tilsvarende `.venv/bin/python` på andre plattformer.
3. Kopier `servers.example.json` til vertens konfigurasjon og erstatt begge plassholdere med faktiske absolutte stier. Eksempelfilen er ikke ferdig konfigurert.
4. Verktøyene `workflow` og `prompt` leser bare egne regler og tillatte rolleprompter. Ukjent rolle avvises. Ingen kandidatfiler, nettsøk eller publisering eksponeres.

Serveren kjører over stdio. Det opprettes ingen nettjeneste. Valgt AI-vert er fortsatt ansvarlig for eventuell modellkjøring og offentlig research.
