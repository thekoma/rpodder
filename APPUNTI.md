 Per riprendere domani

  cd ~/src/rpodder

  # Il server Docker è probabilmente ancora attivo:
  docker compose --profile sqlite ps

  # Se è giù, rialzalo:
  docker compose --profile sqlite up -d rpodder-sqlite

  # Per sviluppare la UI senza rebuild Docker ogni volta:
  cd web && bun run dev    # Svelte dev server su :5173, proxya API a :3005

  # Per Claude Code:
  claude

  Il server gira su http://localhost:3005, il DB è nel volume Docker (persistente).

  Cosa resta da fare (dal TODO)

  - Admin panel — gestione utenti, force feed update, stats
  - SSO/OIDC — autenticazione federata
  - Episode actions history — pagina web per vedere cosa hai ascoltato
  - Test con AntennaPod e gPodder desktop
  - Rate limiting, Prometheus metrics, release builds
claude --resume ee92af91-c56c-4c5a-91c7-c1f297644f0b


claude --resume 146dd172-94d0-4b23-928f-b42662219c67