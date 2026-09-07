# Estado da implementação — 2026-09-07

Escopo: as 15 recomendações do [relatório](melhorias-noctune-2026-09-07.md).
Implementação preparada para a release v1.0.7. O resultado dos workflows e os
artefatos publicados devem ser consultados no GitHub; as verificações abaixo são locais.

| Área | Entrega |
| --- | --- |
| Credenciais | Backend nativo por plataforma, confirmação de escrita, fallback atômico com trava entre processos, migração preservando o original em falhas |
| Fila após rescan | Remapeamento por posição, incluindo duplicatas intencionais e remoção da faixa atual |
| Seek | Acúmulo de pedidos, preservação de pausa e descarte preciso de preroll |
| Spotify | Contratos items/track, legado explícito, paginação restrita ao host, erros HTTP claros; áudio nativo não é anunciado como disponível |
| Atualizador | SHA-256 obrigatório, validação de plataforma/binário, staging e backup com restauração em falha |
| Releases | Cargo.lock, Rust 1.98.0, builds locked, versões consistentes, checksums e attestations no workflow |
| Regressões | 77 testes, incluindo subprocesso de credenciais, PCM sem dispositivo, contratos e servidor HTTP local |
| Carregamento | Workers limitados com substituição de pendências, cancelamento cooperativo, timeout e encerramento de subprocessos |
| Cache de áudio | Leitura por arquivo, publicação atômica, orçamento/expiração e proteção de arquivos em uso |
| Autoplay | Deduplicação de candidatos por caminho |
| Biblioteca | Raízes indisponíveis preservadas durante rescan e aviso ao usuário |
| Onboarding | Setup interativo ou por argumento, doctor JSON sem dados pessoais, teste sonoro opt-in |
| Descoberta | Catálogo compartilhado por ajuda, paleta e exportação de atalhos efetivos; README atualizado |
| Interface | EN/PT-BR nos fluxos centrais, símbolos simples, limites de popup e testes de largura Unicode |
| Serviços | Falhas de persistência e respostas inválidas deixam de parecer sucesso ou catálogo vazio |

## Validação executada neste Windows

- cargo test --locked --all: 77 aprovados; nenhum falhou.
- cargo clippy --locked --all-targets: concluído, com avisos de código morto, estilo e complexidade; não está warning-free.
- cargo fmt --all e git diff --check: concluídos.
- node --check npm/install.js: aprovado.
- Parser do Windows PowerShell para install.ps1: aprovado após corrigir caracteres incompatíveis com leitura sem BOM.
- python scripts/release_manifest.py --check: versões consistentes em 1.0.6.

## Limitações e próximos testes de aceitação

- Não foram usados logins reais do Spotify/Last.fm/Subsonic, nem tocado áudio no dispositivo.
- Persistência do fallback foi testada entre processos; cofres nativos precisam de smoke test por sistema operacional.
- Linux/macOS, scripts de instalação completos e GitHub Actions precisam de execução nos ambientes correspondentes.
- Releases antigas sem sidecar SHA-256 serão recusadas. A nova distribuição exige publicar artefatos pelo workflow atualizado; não há fallback inseguro.
- SHA-256 não autentica um publicador comprometido. O workflow gera attestations, mas o atualizador não as verifica.
- Tradução cobre o catálogo e fluxos centrais, não todas as mensagens das integrações. Símbolos simples não substituem toda a decoração Unicode.
- Preservação de raízes desconectadas cobre rescans na sessão, não restauração completa da biblioteca após reiniciar desconectado.
- Cancelamento é cooperativo: uma operação bloqueada depende do timeout; jobs de serviços fora dos loaders não foram todos unificados.
- A manutenção manual do cache ainda é síncrona; bibliotecas de cache muito grandes precisam de avaliação de latência.
- Layout foi validado com backend de teste; revisão visual interativa permanece necessária.

Essas limitações impedem classificar a implementação como integralmente homologada para produção.
