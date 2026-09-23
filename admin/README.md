# admin/ — admin web (Phase 1 pages 1–3, Phase 2 analytics)

SPA embedded into `meshd`, served at `http://localhost:8080/admin`. Dependency-light (no runtime build server) so `meshd` stays one binary.
Pages: Devices · Models (catalog + "where would this run?" dry-run with reasons) · Plans & Jobs · Analytics. See `docs/ARCHITECTURE-v3-native-mesh.md` §5.
