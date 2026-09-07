# Noctune — melhorias recomendadas
Pesquisa técnica e de produto • 7 de setembro de 2026

**Base:** v1.0.6, commit `2abbe55bfa71e57f5d77729811c207d7b61d6f01`.  
**Público:** mantenedor do projeto. **Decisão:** ordenar o próximo ciclo de desenvolvimento.

## Recomendação principal

Priorizar persistência de credenciais, consistência de reprodução e compatibilidade das integrações. O projeto já oferece muitos recursos; a próxima evolução deve tornar as funções existentes previsíveis e fáceis de descobrir.

A inspeção identificou problemas concretos de lógica e configuração, além de oportunidades de experiência. Os comportamentos interativos não foram reproduzidos nesta pesquisa. As prioridades abaixo são julgamento técnico, não estimativas baseadas em telemetria de usuários.

## Lista de prioridades

P0 = investigar e corrigir antes da próxima publicação; P1 = próximo ciclo; P2 = evolução posterior. Esforço baixo significa mudança localizada; médio envolve mais de um módulo e validação; alto envolve reorganização de estado ou subsistemas. Não são compromissos de prazo.

| # | Melhoria | Prioridade | Esforço | Benefício |
|---|---|---|---|---|
| 1 | Corrigir persistência e migração de tokens | P0 | Médio | Evitar perda de autenticação |
| 2 | Manter identidade da faixa após rescan | P1 | Médio | Evitar saltos e término incorreto da fila |
| 3 | Preservar pausa e intenção durante seek | P1 | Médio | Controles de reprodução previsíveis |
| 4 | Atualizar contrato Spotify e estados de capacidade | P1 | Médio | Integração compatível e mensagens honestas |
| 5 | Tornar atualização verificável e recuperável | P1 | Médio | Reduzir risco de instalação inválida |
| 6 | Fixar dependências das builds publicadas | P1 | Baixo | Reproduzir e investigar versões |
| 7 | Cobrir estados de reprodução com testes | P1 | Médio | Evitar regressões nas próximas releases |
| 8 | Limitar e cancelar carregamentos antigos | P1 | Alto | Reduzir trabalho e processos desnecessários |
| 9 | Dar orçamento próprio ao cache de áudio | P2 | Médio | Controlar disco e memória |
| 10 | Eliminar duplicatas do autoplay infinito | P1 | Baixo | Melhorar continuidade musical |
| 11 | Tratar bibliotecas temporariamente indisponíveis | P2 | Médio | Preservar filas em discos externos/rede |
| 12 | Melhorar primeiro uso e diagnóstico | P2 | Médio | Reduzir configuração por tentativa e erro |
| 13 | Atualizar documentação e descoberta de comandos | P1 | Baixo | Tornar recursos existentes utilizáveis |
| 14 | Validar layouts e unificar idioma | P2 | Médio | Interface consistente em terminais variados |
| 15 | Fazer contratos de serviços falharem explicitamente | P2 | Baixo/médio | Distinguir ausência de dados de erros |

## Achados e critérios de aceite

### 1. Persistência de tokens: prioridade mais alta

**Evidência:** o manifesto declara `keyring = "3"` sem features de armazenamento nativo. A documentação da versão 3.6.3 informa que não há features padrão e que, sem um backend aplicável, a biblioteca usa armazenamento mock. O código interpreta sucesso em `set_password` como persistência e a migração remove o arquivo anterior nessa condição. Isso cria um caminho plausível de perda de credenciais. [Manifesto](https://github.com/WendellOttoni/Noctune/blob/2abbe55/Cargo.toml#L62), [armazenamento e migração](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/secrets.rs#L19), [documentação keyring 3.6.3](https://docs.rs/keyring/3.6.3/keyring/).

**Proposta:** habilitar backend persistente por plataforma; retornar sucesso/erro explícito, incluindo falha do fallback; migrar com verificação e recuperação. Serializar acessos ao fallback e gravar de forma atômica.

**Aceite:** salvar em um processo e recuperar em outro; reiniciar e manter login; falha de armazenamento nunca apagar a única cópia anterior; validar também ambiente Linux sem serviço de segredos.

**Confiança:** alta no desacordo manifesto/documentação; versão transitiva efetiva e comportamento dos binários publicados não foram inspecionados. Confirmar resolução de features antes de atribuir o problema a uma release instalada.

### 2. Identidade da faixa ao atualizar biblioteca

**Evidência:** o rescan remove entradas da fila com `retain`, mas não reconcilia `queue_index`. Com fila A, B, C e B tocando no índice 1, remover A deixa B, C com índice 1 apontando C. O avanço utiliza esse índice. [Aplicação do rescan](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/app/tick.rs#L133), [avanço](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/app/playback.rs#L439).

**Proposta:** identidade estável por entrada da fila e uma operação central para reconciliar reprodução, seleção, prefetch e transições após mutações. Um caminho de arquivo sozinho não distingue repetições intencionais da mesma música.

**Aceite:** remover uma entrada antes da atual mantém a música e o próximo item correto; remover a atual tem comportamento definido; fila vazia não deixa índices pendentes.

### 3. Seek que mantém pausa e acumula comandos

**Evidência:** o seek assíncrono não captura a intenção de pausa; a resposta usa `play_prepared`, que cria um novo sink e inicia o relógio. Seek relativo calcula o destino pela posição atual, sem usar o destino pendente. [Solicitação](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/app/playback.rs#L231), [novo sink](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/audio.rs#L588), [aplicação da resposta](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/app/mod.rs#L833).

**Proposta:** estado explícito de intenção: faixa, posição desejada, pausado/tocando e geração da solicitação. Somar comandos rápidos ao destino pendente.

**Aceite:** seek pausado continua pausado; três avanços de cinco segundos acumulam quinze segundos; trocar de faixa durante carregamento não aplica o resultado anterior. Validar local, HTTP sob demanda e rádio ao vivo separadamente.

### 4. Spotify: compatibilidade antes de ampliar a promessa

**Evidência:** o cliente usa `/playlists/{id}/tracks` e desserializa `track`. A migração oficial do Development Mode troca esse contrato para `items`; apps em Extended Quota Mode estão explicitamente fora dessa migração. Portanto, há incompatibilidade provável para o fluxo pessoal descrito no README, não prova de quebra em todas as contas. [Cliente](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/spotify/api.rs#L195), [mudanças de fevereiro](https://developer.spotify.com/documentation/web-api/references/changes/february-2026), [escopo da migração](https://developer.spotify.com/documentation/web-api/tutorials/february-2026-migration-guide).

O módulo chamado nativo somente altera um booleano e registra uma mensagem; não implementa fluxo de áudio. Deve aparecer como indisponível/experimental até existir implementação verificável. [Sessão nativa](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/spotify/native.rs#L25).

**Proposta:** contratos testados por modo, paginação, tratamento de dados ausentes, mensagens distintas para autenticação, permissão, dispositivo e quota. Informar requisitos do Development Mode: proprietário Premium e limite de usuários conforme a conta. Não repetir o limite antigo de um Client ID: o changelog de julho o elevou para 25 e mudou a contabilização de quotas. [Quota modes](https://developer.spotify.com/documentation/web-api/concepts/quota-modes), [julho de 2026](https://developer.spotify.com/documentation/web-api/references/changes/july-2026).

**Aceite:** fixtures dos formatos suportados; playlist permitida carrega todas as páginas; erro de permissão não aparece como lista vazia; nenhuma indicação de áudio nativo sem reprodução real. Teste autenticado depende de conta e modo reais, não usados nesta pesquisa.

### 5. Atualização verificável e recuperável

**Evidência:** o atualizador aceita resposta HTTP bem-sucedida com bytes não vazios e substitui o executável. A seleção do artefato considera sistema operacional, sem distinguir arquitetura. [Atualizador](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/updater.rs#L76).

**Proposta:** manifesto de artefatos por sistema/arquitetura, digest verificado antes da troca, arquivo temporário validado e recuperação testada. Publicar proveniência verificável para releases e documentar verificação. Um checksum no mesmo canal detecta corrupção, mas sozinho não autentica uma origem comprometida; a política de confiança precisa ser definida. O GitHub oferece atestações e verificação de artefatos. [GitHub — artifact attestations](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations).

**Aceite:** download truncado ou digest divergente não altera a instalação; arquitetura não suportada é recusada; falha de escrita conserva uma versão executável. Não foi demonstrado ataque nem comprometimento.

### 6. Builds reproduzíveis

**Evidência:** `Cargo.lock` está ignorado e os workflows executam Cargo sem `--locked`. Isso permite resoluções diferentes entre builds. O Cargo recomenda versionar o lockfile. [Gitignore](https://github.com/WendellOttoni/Noctune/blob/2abbe55/.gitignore), [release](https://github.com/WendellOttoni/Noctune/blob/2abbe55/.github/workflows/release.yml), [Cargo Book](https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html).

**Proposta e aceite:** versionar lockfile, registrar toolchain/MSRV e usar `--locked` em CI/release. Atualizações de dependências devem produzir uma alteração revisável. Verificar versões consistentes entre Cargo, npm, Scoop e tag. Isso melhora reprodutibilidade de dependências; não garante binários idênticos byte a byte.

### 7. Testes dos comportamentos que importam

**Evidência:** já existe CI em Linux, Windows e macOS com Clippy e testes. Os testes encontrados em reprodução cobrem auxiliares como EXTINF e índices, sem a matriz de estados identificada nesta revisão. [CI existente](https://github.com/WendellOttoni/Noctune/blob/2abbe55/.github/workflows/ci.yml), [testes de playback](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/app/playback.rs#L1021).

**Proposta:** separar decisões de reprodução do dispositivo físico; testar eventos de fim, pausa, erro, rescan, cancelamento, repeat e shuffle com relógio/controlador determinísticos. Usar pequenos arquivos de áudio gerados para testes de decodificação.

**Aceite:** cada correção dos itens 2, 3 e 10 acompanha um caso que falha antes e passa depois. Não propor uma reescrita completa nem apenas aumentar contagem de testes.

### 8. Controle de carregamentos e prefetch

**Evidência:** cada seek cria uma thread; trocar o receptor não cancela o trabalho anterior. No prefetch, uma resposta antiga pode limpar o marcador de carregamento antes da validação do caminho. [Workers](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/app/playback.rs#L271), [prefetch](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/app/mod.rs#L898).

**Proposta:** concorrência limitada, identificador de geração, cancelamento cooperativo e descarte validado antes de alterar marcadores. Consolidar seeks consecutivos e encerrar processos auxiliares quando canceláveis.

**Aceite:** navegação rápida respeita um limite explícito de tarefas; resultados antigos não alteram o estado atual. Medir latência e memória antes/depois. Consumo excessivo é risco inferido, não benchmark observado.

### 9. Cache de áudio com limite e leitura incremental

**Evidência:** o cache de metadados possui poda, mas o download de áudio tem diretório próprio e lê o arquivo completo para memória. Não foi localizada política de orçamento/expiração para esses arquivos. [Download e leitura](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/ytdlp.rs#L236), [poda de metadados](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/cache.rs#L96).

**Proposta:** limite separado de áudio, remoção de itens menos usados, proteção do arquivo em reprodução, escrita atômica e leitura a partir de arquivo quando suportada. Mostrar espaço usado e permitir limpeza pela interface.

**Aceite:** downloads ultrapassando o orçamento liberam espaço de forma previsível; item em reprodução não é removido; arquivo longo não exige duplicar seu conteúdo inteiro em RAM. Não confundir cache técnico com promessa de disponibilidade offline de serviços externos.

### 10. Autoplay sem candidatos duplicados

**Evidência:** quando há menos de três similares, o fallback acrescenta todas as faixas fora da fila, incluindo as já presentes em candidatos. Uma única candidata X pode virar X, X e ser enfileirada duas vezes. [Algoritmo](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/app/playback.rs#L384).

**Proposta e aceite:** deduplicar candidatos por identidade e excluir os já escolhidos do fallback. Testar biblioteca pequena, metadados ausentes e uma única semelhante. Melhorias futuras de diversidade podem considerar histórico e favoritos, após corrigir essa base.

### 11. Biblioteca indisponível não significa música removida

**Evidência:** o scanner ignora diretórios inexistentes e erros de navegação; o rescan remove da fila caminhos não encontrados. Um volume temporariamente desconectado pode ser confundido com remoção. [Scanner](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/app/scan.rs#L31), [reconciliação](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/app/tick.rs#L133).

**Proposta e aceite:** resultado de scan com sucesso/erro por raiz; marcar itens indisponíveis e permitir nova tentativa. Desconectar e reconectar uma unidade não deve destruir a organização da fila. Impacto inferido da combinação das rotas, sem ensaio com disco externo.

### 12. Primeiro uso e diagnóstico guiados

**Oportunidade de produto:** criar um fluxo inicial para escolher pastas, testar saída de áudio e mostrar estado de yt-dlp, FFmpeg e integrações. Um comando de diagnóstico deve indicar problemas e caminhos de configuração, com exportação que remova tokens e URLs sensíveis. O ponto de entrada atual expõe comandos de controle e ajuda, e o download depende de executáveis externos. [CLI](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/main.rs#L45), [dependências de streaming](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/ytdlp.rs).

**Aceite proposto:** em perfil novo, selecionar uma pasta e tocar uma música sem editar TOML; dependência ausente produz uma ação compreensível. Validar com usuários iniciantes antes de investir em mais etapas. Não houve pesquisa de usabilidade com pessoas.

### 13. Documentação e descoberta

**Evidência:** o README ainda descreve early MVP; o enum de ações já contém paleta, letras, Subsonic, Vault e outros recursos. Uma paleta de comandos já existe: ampliá-la e documentá-la é mais útil que criar outra. [README](https://github.com/WendellOttoni/Noctune/blob/2abbe55/README.md), [ações](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/keybinds.rs#L70), [paleta](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/app/input.rs#L829).

**Proposta e aceite:** matriz “disponível / experimental / depende de serviço”, atalhos gerados do mesmo registro que a ajuda, configuração de exemplo e changelog. Toda função anunciada deve ter caminho de uso e pré-requisitos claros. Manter local/radio como percurso inicial simples é uma escolha de produto sugerida.

### 14. Layout, idioma e acessibilidade no terminal

**Evidência:** há mensagens em português e inglês e muitos modais; não foram encontrados testes de snapshots em `src/ui`. Isso sustenta uma proposta de validação, não uma afirmação de que todas as telas quebram. [Modais](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/ui/modals.rs), [mensagens de serviços](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/app/services.rs).

**Proposta:** catálogo de mensagens PT-BR/EN, ajuda contextual, estados identificáveis além de cor e opção de símbolos simples. Testar 80×24, 120×30 e terminal estreito com acentos, CJK e títulos longos. Ratatui documenta snapshots com TestBackend e insta. [Ratatui — testes de snapshots](https://ratatui.rs/recipes/testing/snapshots/).

**Aceite:** foco e ação disponíveis permanecem identificáveis; truncamento respeita largura visual; snapshots cobrem telas críticas. Tamanhos são critérios sugeridos, não requisitos já medidos.

### 15. Erros de serviço que não parecem listas vazias

**Evidência:** o cliente Vault usa `res.json().unwrap_or_default()`: falha de desserialização produz uma lista vazia. O usuário não consegue distinguir catálogo vazio de resposta incompatível. [Vault](https://github.com/WendellOttoni/Noctune/blob/2abbe55/src/vault.rs#L68).

**Proposta e aceite:** propagar erro contextual sem segredos, distinguir vazio/erro/carregando, usar fixtures de respostas e oferecer nova tentativa. JSON inválido deve apresentar falha compreensível; uma lista válida vazia deve continuar sendo estado vazio.

## Sequência sugerida

1. **Confiabilidade:** itens 1, 2, 3 e 10; adicionar os testes correspondentes do item 7.
2. **Release e serviços:** itens 4, 5, 6 e 15; atualizar documentação do item 13.
3. **Uso diário:** itens 8, 9 e 11, medindo memória e latência.
4. **Acabamento:** itens 12 e 14, com ensaio de primeiro uso.

Evitaria assumir agora uma grande implementação de Spotify nativo. Primeiro esclarecer compatibilidade e demonstrar o fluxo de reprodução desejado. Essa decisão exige pesquisa específica de viabilidade e termos; ela não foi tratada como recomendação de implementação neste relatório.

## Alcance e limitações

Pesquisa estática do código atual e consulta a fontes oficiais, com pressuposto de manutenção de um player pessoal multiplataforma. Não inclui implementação, abertura de issues, auditoria exaustiva de segurança, benchmarks, execução de binários de release nem testes autenticados em serviços. A validação de formatação e a CI bem-sucedida foram constatadas na revisão anterior da conversa; esta pesquisa não as substitui por ensaios de funcionamento.

As fontes externas foram consultadas em 07/09/2026. A documentação Spotify de julho foi confrontada com o guia de fevereiro: o limite antigo de Client IDs não foi usado como estado atual. Esforços e ordem são estimativas qualitativas; priorização pode mudar com reprodução dos erros e perfil real dos usuários.

Relatório entregue em Markdown, disponível no ambiente. Verificação estrutural de conteúdo e links; não foi realizada renderização visual em PDF/DOCX.

