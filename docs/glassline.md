# Integração opcional com Glassline

No Windows, o Noctune tenta se conectar em segundo plano ao named pipe local do
Glassline. A ausência ou interrupção do Glassline não altera reprodução, áudio
ou responsividade da TUI.

O worker `src/glassline.rs`:

- tenta reconectar a cada dois segundos;
- envia um snapshot completo após cada conexão;
- publica mudanças de faixa e reprodução e âncoras periódicas de posição;
- recebe somente `previous`, `toggle_playback`, `next` e `show_noctune`;
- entrega os comandos ao loop principal antes de confirmar o resultado;
- limita cada frame a 1 MiB.

O nome e o framing do pipe seguem o contrato mantido no repositório Glassline,
em `docs/noctune-ipc-v1.md`. Todo I/O do pipe fica fora da thread da TUI.
