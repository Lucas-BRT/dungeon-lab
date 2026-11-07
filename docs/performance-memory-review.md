# Auditoria de desempenho e memória

Esta auditoria cobre os structs e enums de `src/`, priorizando o que vive em vetores, o que duplica geometria e o que pode crescer com a quantidade de salas e corredores.

## Como ler os tamanhos

Os números abaixo vêm de `std::mem::size_of` no alvo nativo de 64 bits. Eles medem somente o espaço inline do valor: não incluem alocações de `Vec`, `String`, `Box` ou mapas, nem a capacidade reservada desses contêineres.

| Tipo | Bytes | Observação |
|---|---:|---|
| `Shape` | 32 | Enum de disco ou polígono; os dois payloads maiores têm 24 bytes. |
| `Bounds` | 32 | Dois pontos, sem geometria própria. |
| `Bounded` | 64 | Shape inline mais limites; usado nas regiões de colisão. |
| `ShapeClass` / `Proximity` / `TopologyProfile` / `Role` / `RouteType` | 1 cada | Enumerações compactas. |
| `ByClass` | 40 | Cinco `f64`; tamanho adequado para valores fixos. |
| `TopologySpec` | 48 | Cinco requisitos e o perfil. |
| `ProgressionSpec` | 16 | Dois contadores `usize`; os itens ativos são limitados a oito de cada tipo. |
| `PacingSpec` | 1 | Uma opção booleana. |
| `LeafRoles` | 24 | Três pesos `f64`. |
| `Params` | 488 | Inclui `BTreeMap<String, f64>` para compatibilidade entre formas. |
| `Port` | 40 | Posição, direção, índice de aresta, sala e embutimento. |
| `Rect` | 32 | Retângulo de alocação da sala na grade. |
| `Room` | 184 | Forma, limites, portas, portas usadas e dados de papel. |
| `Node` | 16 | A posição de uma junção é indireta com `Box<V>`. |
| `Edge` | 176 | Inclui duas pontas opcionais, polilinha e vetor da região de colisão. |
| `Candidate` | 168 | Candidato completo usado na busca, com portas, pesos e região. |
| `CandidateView` | 48 | Versão reduzida enviada ao visualizador. |
| `Dungeon` | 784 | Agrega os parâmetros e os vetores do andar. |
| `Metrics` | 208 | Resultados escalares e contagem por classe. |
| `Stage` | 816 | Mantém o `Dungeon`, violações e número de tentativas. |
| `Mulberry` / `Const` | 4 / 8 | Estado dos geradores de números aleatórios. |

## Otimizações aplicadas

- **Uma forma por sala.** Antes, `Room` guardava a forma em `shape` e uma segunda cópia dentro de `bb: Bounded`. Agora guarda `shape` e `Bounds`; isso reduz 32 bytes inline por sala e evita duplicar o vetor de vértices do polígono.
- **Nós menores.** `Node` tinha 24 bytes porque a posição `[f64; 2]` da variante `Junction` era embutida. A posição agora usa `Box<V>`: cada nó ocupa 16 bytes, com uma alocação extra por junção. O JSON continua apresentando `p` como um par de coordenadas.
- **Candidatas de saída menores.** A busca usa `Candidate`; o `Dungeon` guarda `CandidateView`, que mantém somente os IDs e a polilinha usados pela página. O tipo inline cai de 168 para 48 bytes por candidata retida, e regiões de colisão e portas auxiliares deixam de ser mantidas no resultado.
- **Sem clones de candidatas entre fases.** As candidatas vivas são transferidas do cache para a lista estática. Depois que a árvore é montada, os vetores temporários da busca e o cache de pares são liberados antes da fase de ciclos.
- **Opção T sem porta duplicada.** `Action::Branch` guardava a porta e também o índice dessa porta na sala. A ação conserva o índice e recupera o valor quando precisa dele.
- **Matriz contínua.** A matriz densa usada para calcular \(\lambda_2\) agora ocupa um único `Vec<f64>`, evitando uma alocação e um cabeçalho de `Vec` por linha.
- **Payload JSON menor.** `Edge.region` continua disponível para colisão e validação em Rust, mas não é serializada porque o visualizador usa a polilinha `pts`.
- **Atalhos no caso comum.** A validação pula a busca no espaço de estados quando não há travas; o perfil clássico pula cálculo de cortes, hubs e ramos quando não os exige. A métrica de menor ciclo também retorna cedo quando \(\beta=0\).

## Revisão dos demais tipos

### Geometria

`Shape` não é um bom alvo para colocar `Box` somente no polígono: o disco e o vetor de pontos ocupam 24 bytes cada, então essa mudança não reduziria o enum. Colocar as duas variantes atrás de ponteiros diminuiria o tamanho inline, mas adicionaria uma alocação e indireção a cada forma, inclusive às muitas formas temporárias das regiões de corredor.

`Bounded` ainda ocupa 64 bytes e aparece em vetores de regiões de corredor. Ele é um dos próximos alvos a medir. Reduzi-lo exige separar armazenamento geométrico e limites sem espalhar caixas e clones em todas as operações de distância.

### Parâmetros

`ByClass`, `LeafRoles`, `ProgressionSpec` e `PacingSpec` são pequenos valores fixos. `Params` é grande sobretudo por causa de `mix`, que armazena 25 chaves `String` e valores em um `BTreeMap`. Uma matriz `[[f64; 5]; 5]` reduziria alocações e tornaria o acesso mais direto, mas exigiria manter o formato JSON `"classe-classe"` na fronteira de serialização. Como há apenas 25 entradas e a conversão afeta a interface, isso deve ser comparado no benchmark antes de mudar.

### Grafo e busca

`Port` é copiado entre portas possíveis, portas usadas, pontas de aresta e candidatas. Os campos `room` e `edge` podem ser redundantes quando a porta está armazenada dentro de uma sala, mas uma porta pode ser deslocada continuamente para alinhar um corredor. Substituí-los por índices exige preservar esse caso sem perder a posição exata.

`Edge` é grande e contém geometria que deve continuar viva durante a validação. `ends` pode ser simplificado se o modelo de porta for refeito como uma referência compacta, mas as portas deslocadas impedem trocar cada valor por um simples índice sem mudança semântica. `region` não é enviada ao navegador.

`Candidate` é grande por projeto enquanto a busca está ativa: os pesos, portas e regiões são necessários para avaliar e aplicar ações. A lista final usa o tipo menor `CandidateView`. `Dungeon` e `Stage` carregam esses valores por composição; colocar o andar atrás de `Box` reduziria o tamanho do valor pai, mas não a memória total e adicionaria uma alocação.

`Action`, `Opt`, `SegRef`, `Changes` e `Block` são auxiliares temporários. `Action::Branch` perdeu o campo duplicado; `Block` é um valor `Copy` de quatro inteiros. `Gen` concentra caches e mapas temporários, e a liberação entre fases evita manter esses dados até a serialização.

### Métricas, validação e infraestrutura

`Metrics` contém principalmente números e o mapa final de contagens. A matriz de Laplace, que não faz parte do resultado, agora usa memória contígua. `Tarjan` conserva vetores de visitação por nó; essa memória é linear no tamanho do grafo e necessária para pontes e articulações.

O `Arc` residual do fluxo e o `State` de progressão são internos. O espaço de estados é limitado por oito bits de chave e oito de interruptor; no pior caso há até \(2^{16}\) combinações por nó. Só se percorre esse espaço quando há ao menos uma trava.

`SpatialHash` usa um `HashMap` de células e vetores de índices. Essa representação evita comparar todo par geométrico em mapas maiores; o overhead de buckets deve ser medido em mapas pequenos e grandes antes de trocá-la.

`Answer` contém referências ao resultado e as métricas. `Options` guarda host e argumentos do servidor. `Mulberry` e `Const` são estados pequenos. Nenhum deles domina o uso de memória do gerador.

`Rect` ocupa 32 bytes dentro de cada sala e não é usado pelo código atual da página. Ele ainda sai no JSON como `box` e pode ser útil para outros consumidores. Uma etapa futura pode substituí-lo por coordenadas inteiras do bloco ou removê-lo depois de confirmar que nenhum consumidor externo depende desse campo.

## Estrutura da API Rust

Os métodos associados já aparecem onde há estado ou regras que pertencem a um tipo: `Gen` mantém a busca e os caches; `SpatialHash` controla seus buckets; `TopologySpec::requirements` transforma o perfil em requisitos efetivos. As operações em `geom` são funções puras sobre formas e vetores, então agrupá-las em métodos não traria por si só mais segurança.

O ponto mais promissor para uma API tipada é `topology`: hoje funções como `shortest_path` recebem separadamente `node_count`, `edges`, `start` e `goal`, e cada uma pode reconstruir a adjacência. Um `NavigationGraph` ou `GraphView` poderia validar os endpoints uma vez, manter a adjacência e expor métodos como `shortest_path`, `edge_connectivity` e `cycle_girth`. `metrics` e `validate` poderiam compartilhar essa visão. É uma refatoração maior e deve ser comparada no Criterion, pois manter a adjacência também tem custo de memória.

## Benchmarks Criterion

O benchmark nativo mede separadamente a geração (`generate`), geração com validação e tentativas (`stage_with_validation`), validação isolada e cálculo de métricas. Inclui os níveis 1–5, um cenário com grade irregular, formas giradas e rotas W, e um cenário de espinha dorsal com chaves, interruptor e ritmo ativados.

```sh
cargo bench --bench generator
```

Para guardar e comparar uma referência local:

```sh
cargo bench --bench generator -- --save-baseline antes
# depois de uma otimização:
cargo bench --bench generator -- --baseline antes
```

Criterion guarda os dados e relatórios sob `target/criterion`. O benchmark mede código Rust nativo; ele não mede serialização WebAssembly, transferência JSON nem desenho no navegador. Para memória heap e contagem de alocações, o próximo passo é acrescentar um profiler como DHAT ou heaptrack e comparar com estes mesmos cenários.

Com a biblioteca publicada como `cdylib` e `rlib`, o Cargo atual mostra um aviso de colisão de nomes de artefatos ao compilar o benchmark. A execução termina normalmente. Se uma versão futura do Cargo transformar esse aviso em erro, a saída do benchmark precisará ser movida para um pacote nativo separado.

Uma execução curta durante esta auditoria, com 10 amostras e 0,5 s de coleta, estimou: geração dos níveis 1–5 em 2,60 ms, 4,14 ms, 15,66 ms, 26,19 ms e 21,15 ms; cenário estendido em 91,68 ms; cenário de progressão no nível 5 em 34,33 ms; validação do nível 5 em 573 µs; métricas em 61,5 µs. Esses números são uma referência inicial do alvo nativo atual, não uma comparação antes/depois nem uma medição de precisão máxima. Para decisões pequenas, use a coleta padrão mais longa e repita em uma máquina ociosa.
