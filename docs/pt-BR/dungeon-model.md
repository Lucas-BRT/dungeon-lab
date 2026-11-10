# Modelo matemático de geração de níveis

Este documento organiza a matemática do gerador de dungeons em torno de uma pergunta simples: **como construir um mapa que seja espacialmente válido, conectado e interessante de percorrer?**

A resposta tem três partes. Primeiro definimos as salas e os corredores no plano. Depois representamos as conexões como um grafo e medimos sua topologia. Por fim, escolhemos quais propriedades devem ser obrigatórias e quais são apenas preferências.

Esta é a especificação matemática por trás do gerador do Tapiti. Como a implementação em Rust não cobre todas as ideias do modelo, o texto distingue o **modelo de referência**, o **comportamento atual do crate** e as **extensões ainda propostas**.

## 1. O problema em uma frase

Um nível é uma rede de salas e passagens, colocada no espaço, que deve respeitar restrições geométricas e oferecer uma experiência de exploração com uma estrutura desejada.

Usaremos a seguinte representação:

$$
\mathcal D = (H, \varphi, \alpha, \Lambda)
$$

- $H=(V,E)$ é o **grafo navegável**: vértices são salas ou junções; arestas são trechos de corredor entre esses pontos.
- $\varphi$ é a **realização geométrica**: informa onde ficam as salas e por onde passam os corredores.
- $\alpha$ associa funções às partes do nível, como início, escadas, prêmio ou emboscada.
- $\Lambda$ reúne os parâmetros do gerador, como dimensões, folgas, pesos de forma e densidade.

O gerador atual constrói um piso 2D conectado. A representação separa a topologia — quem se conecta a quem — da geometria — como essas conexões cabem no plano. Essa separação permite especificar o formato de uma rede sem exigir que ela já tenha coordenadas.

### Vocabulário e status

Neste texto, **modelo de referência** identifica conceitos preservados da especificação matemática original; **crate atual** indica o que existe nesta implementação Rust; **proposta** identifica uma extensão ainda não implementada. Perfis topológicos, chaves, interruptores e ritmo de exploração agora fazem parte do crate; campanhas com vários pisos e objetivos de missão continuam como propostas.

## 2. Notação básica

| Símbolo | Significado |
|---|---|
| $\mathbb R^2$ | plano do piso, em pixels |
| $S_i\subset\mathbb R^2$ | região ocupada pela sala $i$ |
| $\partial S_i$ | contorno da sala $i$ |
| $\operatorname{dist}(A,B)$ | menor distância euclidiana entre dois conjuntos |
| $w$ | largura do corredor |
| $\sigma$ | afastamento mínimo entre regiões que não se conectam |
| $\ell_s$ | comprimento mínimo do trecho reto logo após uma porta |
| $\ell_{seg}$ | comprimento mínimo de um segmento de corredor |
| $\ell_{min}$ | distância mínima desejada entre salas conectadas |
| $\lfloor x\rfloor$ | maior inteiro menor ou igual a $x$ |

Uma folga é uma distância entre **bordas de regiões**, não entre seus centros. Por exemplo, exigir $\operatorname{dist}(S_i,S_j)\geq 2g$ significa que há pelo menos $2g$ pixels de espaço entre os contornos das duas salas.

## 3. Geometria das salas

### 3.1 Alfabeto de direções

Portas e segmentos de corredor usam um conjunto discreto de direções:

$$
\Theta_m=\left\{\frac{2\pi k}{m}: k=0,\ldots,m-1\right\},
\qquad m\in\{4,8\}.
$$

Com $m=4$, as direções são horizontal e vertical. Com $m=8$, acrescentam-se as diagonais de 45°. Chamamos o primeiro caso de **retilíneo** e o segundo de **octilinear**.

Uma curva livre poderia ligar quaisquer dois pontos, mas as direções discretas tornam as passagens mais previsíveis e facilitam testar sua largura e suas interseções. Essa é uma simplificação deliberada do modelo.

### 3.2 Classes de forma

Cada sala ocupa uma região $S_i$ de uma classe $\kappa_i$:

$$
\mathcal S=\{\textsf{rect},\textsf{disc},\textsf{oct},\textsf{L},\textsf{star}\}.
$$

| Classe | Parâmetros principais | Área ou construção |
|---|---|---|
| Retângulo | largura $W$, altura $H$ | $WH$ |
| Disco | raio $\rho$ | $\pi\rho^2$ |
| Octógono | circunraio $\rho$ | $4\rho^2\sin(\pi/4)$ |
| L | caixa $W\times H$, entalhe $a\times b$, canto $c$ | $WH-ab$ |
| Estrela truncada | número de pontas $n$, raios e largura do topo $\tau$ | calculada pela fórmula do polígono (*shoelace*) |

Uma estrela tem as pontas cortadas. Sem o corte, cada ponta seria apenas um vértice e não haveria um trecho de borda adequado para abrir uma porta reta.

No modelo de referência, uma sala é encaixada num retângulo útil $\beta$ por uma função

$$
\operatorname{fit}_{\kappa}(\beta,\xi)\to S\subseteq\beta,
$$

onde $\xi$ representa escolhas aleatórias como proporções e orientação. No crate, os blocos do [andaime](../../crates/tapiti-core/src/generate.rs:1165) também limitam onde as salas podem ficar.

Para manter portas alinhadas com o alfabeto, a especificação de referência restringe as rotações das formas a $\varphi\in\Theta_m$. Isso é uma escolha de modelagem: rotações contínuas exigiriam aceitar também paredes cujas normais não pertencem ao alfabeto de corredores.

## 4. Portas e capacidade das salas

Uma porta é um par $p=(x,\theta)$: o ponto $x$ no contorno da sala e a direção $\theta$ para a qual o corredor sai.

Numa aresta de polígono, nem todo ponto é admissível. A porta precisa ficar afastada dos cantos e sua saída não pode atravessar a própria sala. Definimos o recuo

$$
\iota=\frac{w}{2}+\sigma.
$$

Para uma aresta de comprimento $L$, os pontos admissíveis ficam inicialmente no intervalo $[\iota,L-\iota]$. No modelo matemático, qualquer ponto desse intervalo pode ser escolhido; a implementação amostra posições em passos `portStep` e ainda ajusta portas quando resolve algumas interseções.

A faixa de saída de uma porta pode ser representada como

$$
F(x,\theta)=\{x+s\,u(\theta)+r\,u(\theta+\tfrac\pi2):
0\leq s\leq\ell_s,\ |r|\leq w/2\},
$$

onde $u(\theta)=(\cos\theta,\sin\theta)$. Intuitivamente, $F$ é o retângulo ocupado pelo corredor logo ao sair da parede.

Duas portas usadas na mesma sala devem manter suas faixas separadas:

$$
\operatorname{dist}(F(p_i),F(p_j))\geq\sigma\quad(i\ne j).
$$

Chamamos de **capacidade** da sala o maior número de portas que satisfaz essa condição:

$$
\operatorname{cap}(S)=\max\{|U|: U\text{ é um conjunto de portas admissíveis e separado}\}.
$$

Para uma aresta com intervalo admissível de comprimento $\lambda$, o limite é $\lfloor\lambda/(w+\sigma)\rfloor+1$. Num disco de raio $\rho$ com portas igualmente espaçadas, a distância entre posições vizinhas é a corda $2\rho\sin(\pi/q)$; todas as $q$ portas cabem se essa corda for ao menos $w+\sigma$. Essa conta mostra por que o raio limita o número de portas, mesmo quando o parâmetro pede mais direções.

Por exemplo, num mesmo lado reto, duas portas não podem ficar mais próximas do que aproximadamente $w+\sigma$. A capacidade, portanto, depende tanto do tamanho da sala quanto da largura dos corredores.

## 5. Corredores

### 5.1 Corredor como região ocupada

Um corredor é formado por segmentos conectados. Cada segmento segue uma direção de $\Theta_m$ e tem comprimento de pelo menos $\ell_{seg}$. O segmento que parte de uma porta também precisa manter a direção da porta por pelo menos $\ell_s$.

Se $\gamma_e$ é a linha central de um segmento $e$, sua faixa ocupada é a vizinhança dessa linha com raio $w/2$. A região de um corredor é a união das faixas de seus segmentos e dos remates nas dobras e junções. Assim, as verificações de colisão comparam regiões, e não apenas linhas centrais.

### 5.2 Formas de ligação

Entre duas portas, a especificação considera três tipos de rota:

| Tipo | Dobras | Descrição |
|---|---:|---|
| I | 0 | segmento reto entre portas alinhadas e voltadas uma para a outra |
| L | 1 | duas semirretas que se encontram à frente das portas |
| Z/U | 2 | dois trechos de saída ligados por um segmento intermediário |

Por exemplo, para uma rota reta entre $(p_a,\theta_a)$ e $(p_b,\theta_b)$, é necessário que as portas se encarem, que $p_b-p_a$ esteja alinhado com a direção de saída e que a distância permita os dois trechos mínimos:

$$
\theta_b\equiv\theta_a+\pi\pmod{2\pi},\qquad
(p_b-p_a)\times u(\theta_a)=0,\qquad
(p_b-p_a)\cdot u(\theta_a)\geq 2\ell_s.
$$

As rotas L e Z impõem condições semelhantes de alinhamento, direção e comprimento mínimo para cada segmento. No [gerador Rust](../../crates/tapiti-core/src/generate.rs:345), as rotas diretas seguem a mesma família I/L/Z, com limite de dobras controlado por `maxBends`.

Para uma rota L, sejam $u_a=u(\theta_a)$, $u_b=u(\theta_b)$, $d=p_b-p_a$. Se $u_a\times u_b\neq0$, o encontro das semirretas ocorre nos parâmetros

$$
t=\frac{d\times u_b}{u_a\times u_b},\qquad
s=\frac{d\times u_a}{u_a\times u_b}.
$$

A rota só é viável se $t,s\geq\ell_s$ e se a dobra formada pelas direções for permitida. Para Z, escolhemos uma direção intermediária $u(\delta)$ e resolvemos

$$
p_b-p_a=t_a u_a+t_c u(\delta)-t_bu_b,
\qquad t_a,t_b\geq\ell_s,\quad t_c\geq\ell_{seg}.
$$

O sinal de menos aparece porque $u_b$ aponta para fora da segunda sala; o último segmento da rota chega à porta na direção oposta. São duas equações vetoriais em três comprimentos: para cada direção intermediária há uma família de soluções. A especificação escolhe um comprimento ou uma razão entre trechos e resolve os demais.

### 5.3 Junções

Uma junção conecta três braços (T) ou quatro braços (X). Os braços compartilham um ponto de encontro, que também é um vértice do grafo navegável. Essa representação é importante para contar caminhos e ciclos: uma junção T é uma bifurcação, não um triângulo de passagens.

A especificação de referência descreve ainda uma **condutância** $\omega_h$ entre braços de uma junção. Ela pode modelar se dois braços se comportam como um único espaço ou como regiões parcialmente acopladas. O campo `junction` aparece nos parâmetros, mas está marcado no código como não usado pelo gerador; portanto, essa interpretação é uma extensão do modelo de espaços, não um efeito que o crate atual produza.

### 5.4 Custo e peso de uma rota

Uma rota pode ser geometricamente válida e ainda assim ser menos desejável que outra. Para ordenar candidatas, usamos um custo estético:

$$
C(k)=c_b b(k)+c_\ell\ell(k)+c_{45}n_{45}(k)
-c_f\,\mathbf 1_{\text{portas frente a frente}},
$$

onde $b(k)$ é o número de dobras, $\ell(k)$ o comprimento e $n_{45}(k)$ o número de dobras de 45°. A indicação $\mathbf 1$ vale 1 quando a condição é verdadeira e 0 quando não é.

O custo não decide validade. Entre candidatas válidas, o peso é

$$
\omega(k)=M_{\kappa_a\kappa_b}\,p_{tipo(k)}\,e^{-C(k)},
$$

onde $M$ é a compatibilidade entre as classes das salas e $p_{tipo(k)}$ é o peso do tipo de rota. Um peso zero proíbe a ligação; um peso maior aumenta sua chance numa escolha ponderada. Esse é o mecanismo usado para variar escolhas sem abandonar as restrições geométricas.

## 6. Grafo navegável

### 6.1 A representação usada para topologia

O grafo refinado do nível é

$$
H=(V,E),\qquad V=R\cup J,
$$

onde $R$ é o conjunto de salas, $J$ o conjunto de junções e cada aresta em $E$ é um braço de corredor entre uma sala e uma junção, duas junções ou duas salas. O grafo pode ter arestas paralelas: duas passagens fisicamente distintas entre os mesmos vértices continuam sendo duas arestas.

Usamos $H$ como representação principal para conectividade, ciclos e rotas independentes. Uma projeção que liga toda sala de uma junção a todas as outras salas dessa junção pode ser útil para certas medidas de “saltos entre salas”, mas cria conexões artificiais se usada para contar ciclos.

**Exemplo.** Três salas ligadas por uma única junção T formam uma árvore em $H$: três braços e três salas, sem ciclo. Se projetarmos o T como ligações entre cada par de salas, aparece um triângulo. Esse triângulo não corresponde a três passagens independentes; todas usam a mesma junção. Por isso a clique-expansão não deve ser usada para calcular $\beta$ ou rotas edge-disjoint.

### 6.2 Conectividade

Um nível conectado tem um caminho entre qualquer par de salas. Se $c(H)$ é o número de componentes conexos, a condição geral é $c(H)=1$ para um piso que contém uma única região jogável.

Uma **ponte** é uma aresta cuja remoção desconecta o grafo. Um **vértice de articulação** é um vértice cuja remoção o desconecta. Essas duas estruturas identificam gargalos: lugares pelos quais a exploração precisa passar.

### 6.3 Número de ciclos

O número independente de ciclos, também chamado **número ciclomático**, é

$$
\beta(H)=|E|-|V|+c(H).
$$

Para um piso conectado, $\beta=|E|-|V|+1$. Uma árvore tem $\beta=0$. Acrescentar uma ligação entre dois pontos já conectados aumenta $\beta$ em 1.

**Exemplo.** O caminho sala A — sala B — sala C tem 3 vértices e 2 arestas, logo $\beta=2-3+1=0$. Ao adicionar uma passagem entre A e C, há 3 arestas e 3 vértices; então $\beta=3-3+1=1$, correspondente a um ciclo independente.

Se $H$ é planar e conectado, a fórmula de Euler dá $|V|-|E|+|F|=2$. Portanto, $\beta=|F|-1$: é o número de faces limitadas do desenho. Essa interpretação depende da planaridade; o número ciclomático continua definido mesmo quando não se usa a leitura por faces.

### 6.4 Ciclos não garantem rotas alternativas até a saída

O número de ciclos descreve a rede inteira, mas não onde os ciclos ficam. Para medir rotas independentes entre início $s$ e escadas $t$, usamos

$$
\lambda_H(s,t)=\text{máximo número de caminhos de }s\text{ a }t
\text{ que não compartilham arestas}.
$$

Pelo teorema de Menger, esse número também é o tamanho mínimo de um conjunto de arestas cuja remoção separa $s$ de $t$.

Assim, $\beta(H)>0$ **não** implica $\lambda_H(s,t)>1$. Por exemplo, considere o caminho $s-a-t$ e uma ala em ciclo $a-b-c-a$. O grafo tem 5 vértices e 5 arestas, então $\beta=5-5+1=1$, mas toda rota de $s$ a $t$ ainda usa a aresta $a-t$: $\lambda_H(s,t)=1$. Já duas rotas $s-a-t$ e $s-b-t$ têm $\beta=1$ e $\lambda_H(s,t)=2$. O perfil do nível precisa especificar separadamente “quantos ciclos existem” e “quantas rotas chegam à saída”.

Outras medidas úteis incluem:

| Medida | Definição | O que descreve |
|---|---|---|
| Distância por salas $d_{room}(s,t)$ | caminho mínimo que conta cada chegada a uma sala como 1 e a passagem por junção como 0 | extensão da rota principal em salas atravessadas |
| Distância geométrica $d_{geom}(s,t)$ | menor soma dos comprimentos de corredor entre início e saída | percurso físico mínimo |
| Diâmetro | maior distância mínima entre pares de salas | tamanho global percebido |
| Girth $g(H)$ | tamanho do menor ciclo (definido como $+\infty$ se não houver ciclo) | presença de ciclos curtos |
| Grau $d(v)$ | número de arestas incidentes em $v$ | quantidade local de opções |
| Articulações e pontes | vértices/arestas cuja remoção desconecta $H$ | gargalos estruturais |
| $\lambda_H(s,t)$ | rotas edge-disjoint entre início e saída | redundância da rota final |
| $\lambda_2$ | segundo menor autovalor do Laplaciano $L=D-A$ | conectividade global, de forma agregada |

### 6.5 Quando um nível é válido

As restrições abaixo separam problemas diferentes. **Conexidade** fala da rede; **folga** fala da geometria; **acessibilidade** fala das regras de jogo.

- **V1 — Conexidade.** Todos os vértices navegáveis relevantes pertencem à mesma componente de $H$.
- **V2 — Folgas.** Salas mantêm a distância mínima $2g$; o corredor permanece a pelo menos $\sigma$ das salas às quais não se conecta; dois corredores separados mantêm a folga, exceto numa junção válida. Entre salas ligadas, também se exige a distância direta $\ell_{min}$, para que salas próximas na diagonal não se tornem quase adjacentes apesar de um caminho longo.
- **V3 — Portas.** Cada porta usada é admissível para sua sala, e as faixas de saída das portas da mesma sala ficam separadas por pelo menos $\sigma$.
- **V4 — Corredores.** Direções, dobras e comprimentos respeitam $\Theta_m$, $\ell_s$, $\ell_{seg}$ e o limite de dobras.
- **V5 — Início e saída.** Há uma sala de início e outra de escadas, distintas; as escadas respeitam o limite de portas e ficam a pelo menos `minHops` saltos de sala do início.
- **V6 — Mobília.** Na especificação original, pedras e cristais também precisam ficar fora das faixas de saída e manter distância entre si. Essa etapa pertence à mobília do jogo e não é implementada pelo gerador geométrico deste crate.

Um nível precisa satisfazer todas as restrições que se aplicam ao seu escopo. Um custo estético menor não pode compensar uma violação de V1–V5.

## 7. Densidade de ciclos

Para normalizar o número de ciclos, a especificação define uma referência $\beta_{ref}$ e uma densidade:

$$
\delta_{alvo}=
\begin{cases}
\beta^*/\beta_{ref}, & \beta_{ref}>0,\\
0, & \beta_{ref}=0.
\end{cases}
\qquad 0\leq\delta_{alvo}\leq1.
$$

Na interface e no código, essa referência pode aparecer com o nome `betaMax`. É importante interpretar o nome com cuidado: o valor é obtido por uma estratégia gulosa que acrescenta as ações válidas de maior peso até não haver mais opções. Isso produz uma **referência maximal da heurística**, não uma prova do máximo global entre todas as configurações possíveis.

O alvo é arredondado de $\delta_{alvo}\beta_{ref}$. Por exemplo, se $\beta_{ref}=8$ e $\delta_{alvo}=0{,}25$, o alvo será cerca de 2 ciclos. A densidade realizada é $\delta_{real}=\beta(H)/\beta_{ref}$ quando $\beta_{ref}>0$; ela pode diferir do alvo por arredondamento, por uma ação X que acrescenta dois ciclos ou porque $\beta_{ref}$ vem de uma heurística, não de um máximo provado. Em certos casos, $\delta_{real}$ pode até passar de 1. O alvo controla a quantidade aproximada; perfis topológicos são necessários para controlar a localização e a função desses ciclos.

Para um grafo conectado, o grau médio satisfaz

$$
\langle d\rangle=\frac{2|E|}{|V|}
=\frac{2(|V|-1+\beta)}{|V|}.
$$

Essa identidade relaciona o número de ciclos ao grau médio, mas não determina o desenho da rede.

## 8. Geração do modelo de referência

O processo pode ser entendido como seis etapas. A especificação original descreve esta sequência; o crate Rust implementa uma variante, resumida na seção 12.

1. **Andaime.** Criar uma grade de $C\times R_g$ células e combiná-las em blocos. Algumas células podem ser descartadas, desde que os blocos restantes continuem conectados.
2. **Salas.** Sortear uma classe de forma para cada bloco e encaixar a região no espaço útil, respeitando limites e folgas.
3. **Candidatos.** Escolher pares de salas a considerar; para cada par, procurar portas e rotas viáveis.
4. **Conflitos.** Marcar pares de candidatos que não podem coexistir por colisão geométrica ou uso de portas muito próximas.
5. **Árvore e ciclos.** Primeiro conectar as salas por uma árvore geradora. Depois acrescentar ligações entre pontos já conectados até aproximar-se do alvo de $\beta$.
6. **Salas-folha e papéis.** Usar células descartadas para salas de grau 1 e atribuir papéis como início, escadas, prêmio ou armadilha.

Uma árvore garante conectividade sem ciclos. A segunda fase acrescenta alternativas sem remover as conexões existentes.

### 8.1 Mistura das classes de sala

Uma distribuição de classes pode considerar tanto o tamanho do bloco quanto as classes dos blocos vizinhos. Seja $N(i)$ o conjunto de blocos adjacentes a $i$, $n_i$ o número de células de $i$, e $n_\kappa(i)$ quantos vizinhos têm classe $\kappa$. A regra de amostragem é

$$
P(\kappa_i=\kappa\mid\kappa_{N(i)})
\propto w_\kappa(n_i)\exp\bigl(h\,n_\kappa(i)\bigr).
$$

O peso $w_\kappa(n_i)$ pode, por exemplo, impedir estrelas em blocos pequenos. O parâmetro $h$ controla a homofilia: $h>0$ favorece vizinhos da mesma classe; $h<0$ favorece classes diferentes. No crate, essa regra é amostrada em quatro varreduras e depois são aplicadas as cotas `maxOf`.

### 8.2 Pares de salas candidatos

A especificação de referência considera grafos de proximidade construídos a partir dos centros das salas:

- **RNG**: uma ligação entre $i$ e $j$ é removida se outra sala estiver mais próxima de ambas;
- **Gabriel**: uma ligação é removida se outra sala cair dentro do disco cujo diâmetro é o segmento entre os centros de $i$ e $j$;
- **Delaunay limitado**: considerar vizinhos de Delaunay até uma distância máxima.

Se $c_i$ é o centro da sala $i$ e $d_{ij}=\|c_i-c_j\|^2$, o teste RNG mantém $\{i,j\}$ quando não existe $k$ tal que $\max(d_{ik},d_{jk})<d_{ij}$. O teste de Gabriel mantém a ligação quando não existe $k$ com $\|c_k-(c_i+c_j)/2\|^2<d_{ij}/4$. A variante `Near` usada no crate mantém pares com $\|c_i-c_j\|\leq r$.

Esses filtros controlam quais pares entram na busca. Eles não garantem por si só que as salas possam ser conectadas: ainda é necessário encontrar portas e corredores sem colisão.

### 8.3 Grafo de conflitos

Seja $\mathcal C$ o conjunto de ligações candidatas. Construímos um grafo de conflitos $X=(\mathcal C,F_X)$, em que dois candidatos estão ligados em $F_X$ quando não podem ser escolhidos juntos.

Uma seleção sem conflitos é um **conjunto independente** de $X$. Para formar um nível conectado, também exigimos que a seleção construa um grafo navegável conectado. Essa formulação separa duas perguntas:

1. cada ligação é válida sozinha?
2. as ligações escolhidas podem coexistir?

### 8.4 Árvore, ciclos e salas-folha

O crate escolhe uma sala inicial e cresce uma árvore geradora. Em cada passo, uma ligação candidata precisa conectar uma sala já alcançada a uma sala nova. A escolha é ponderada pelos pesos das candidatas. Se a lista atual não permite avançar, o gerador amplia o conjunto de pares de proximidade e tenta continuar.

Depois de conectar as salas, o gerador constrói uma referência `betaMax` preenchendo uma cópia do grafo com ações válidas de maior peso. Essa busca considera ligações diretas e ações de junção T/X. A referência é gulosa: depende das escolhas anteriores e não prova o máximo global.

Na fase real de ciclos, o alvo é $\beta^*=\operatorname{round}(\delta_{alvo}\beta_{ref})$, onde `delta` é o parâmetro do crate. São sorteadas ações válidas até alcançar ou ultrapassar esse alvo, ou até não haver mais ações. Uma ligação direta ou T acrescenta um ciclo independente; um cruzamento X pode acrescentar dois, pois fecha um novo caminho e divide o corredor cruzado em dois ramos. Por isso $\beta^*$ é um alvo, não uma garantia de igualdade exata.

As câmaras-folha são acrescentadas depois: cada uma é uma sala ligada por um único corredor. No grafo, a operação acrescenta um vértice e uma aresta, logo $\Delta\beta=0$. Ela aumenta o número de becos sem aumentar o número de ciclos. Os papéis de início, escada, chefe, descanso, emboscada, armadilha, prêmio e sala vazia descrevem conteúdo. Chaves, interruptores e travas são dados de progressão separados dos rótulos.

No crate, o início é escolhido entre as salas vivas. Depois da árvore, as escadas são escolhidas entre as folhas mais distantes do início; se a distância mínima `minHops` não for atingida, `stage` pode tentar outra semente até o limite de tentativas.

## 9. Perfis topológicos

O parâmetro `topology` permite pedir uma **intenção topológica** explicitamente. O crate implementa requisitos obrigatórios de ramos, profundidade, rotas independentes até a saída, grau de hub e comprimento mínimo de ciclo. Se a geometria não permitir cumprir um requisito, o gerador registra a limitação e o validador aponta a violação.

Representamos um perfil como

$$
\Pi=(C_{hard},\mathbf t,\mathbf w),
$$

onde:

- $C_{hard}$ é um conjunto de condições obrigatórias;
- $\mathbf t$ é o vetor de valores-alvo para as medidas do grafo;
- $\mathbf w$ define a importância relativa de cada alvo.

Seja $\Phi(H)$ o vetor de métricas, por exemplo

$$
\Phi(H)=
(\beta,\ g(H),\ \lambda_H(s,t),\ d_{room}(s,t),\ d_{geom}(s,t),\
\text{pontes},\text{articulações},\text{graus},\text{ramos}).
$$

O conjunto viável contém apenas níveis que satisfazem a validade geométrica e as condições obrigatórias codificadas. O crate ainda não procura o melhor nível segundo uma função global de pontuação. A expressão abaixo formaliza uma extensão possível para comparar métricas com alvos:

$$
J_{\Pi}(H)=\sum_i w_i\,\rho_i(\Phi_i(H),t_i),
$$

onde $\rho_i$ penaliza o afastamento do alvo. As restrições obrigatórias filtram os níveis; a função $J_\Pi$ classificaria as opções restantes. Essa otimização global continua fora do crate atual.

### 9.1 Exemplos de perfis

**Espinha com alas opcionais.** Escolhemos um caminho principal $P$ de $s$ até $t$. Removemos de $H$ os vértices de $P$ e examinamos cada componente restante $C$. Definimos sua vizinhança na espinha por

$$
\partial_P C=\{e\in E:e\text{ tem uma extremidade em }C\text{ e a outra em }V(P)\}.
$$

Se $|\partial_P C|=1$, $C$ é um ramo lateral simples: há uma única aresta que liga o componente à espinha. Se $|\partial_P C|\geq2$, $C$ oferece um desvio que volta à espinha — um atalho ou ciclo, não um ramo simples. Contamos arestas, não apenas vértices de contato: duas passagens paralelas para o mesmo vértice de $P$ já formam uma segunda ligação e não devem ser classificadas como ramo simples.

**Duas rotas até a saída.** Exigir $\lambda_H(s,t)\geq2$. Isso garante que nenhuma única aresta seja suficiente para separar início e saída.

**Anel explorável.** Exigir um ciclo simples $C\subseteq H$ com pelo menos $L_{loop}$ arestas. A condição $\beta\geq1$ sozinha permitiria que o único ciclo fosse muito pequeno.

**Hub.** Exigir uma sala ou junção $h$ com grau $d(h)\geq d_{hub}$, ou favorecer uma distribuição de graus que concentre várias opções numa região.

**Gargalo intencional.** Exigir uma ponte ou articulação num trecho especificado do caminho principal. Isso pode concentrar o ritmo do nível, desde que a progressão permita atravessá-la.

Esses requisitos podem coexistir. Por exemplo, um perfil pode exigir uma espinha longa, dois ramos com recompensas e duas rotas alternativas no trecho final.

## 10. Progressão com chaves e interruptores

Topologia responde “quais lugares se conectam?”. Progressão responde “quando o jogador pode atravessar cada conexão?”. Para modelar portas trancadas e objetivos, ampliamos o estado do jogador:

$$
q=(I,S,O),
$$

onde $I$ é o inventário, $S$ o conjunto de mecanismos ativados e $O$ os objetivos concluídos.

Uma aresta pode ser atravessada somente se sua condição for satisfeita em $q$. O crate implementa máscaras de até oito chaves e oito interruptores e verifica o espaço de estados de localização e inventário. Uma chave é recolhida ao entrar na sala; um interruptor é ativado ao entrar nela.

Para manter cada trava significativa mesmo quando há atalhos, o gerador divide o grafo por cortes sucessivos do caminho principal e aplica a condição a todas as arestas que atravessam cada corte. Assim, não basta colocar uma chave antes de uma porta escolhida: todo caminho que ultrapasse aquele ponto deve satisfazer a condição. O validador percorre estados $(v,I,S)$ e exige que a escada continue alcançável.

O campo `pacing.enabled` distribui salas de descanso, chefe e emboscada ao longo do caminho principal e aumenta a preferência por prêmios nos ramos opcionais. Esses papéis descrevem ritmo e conteúdo; não criam objetivos de missão.

O mesmo modelo pode se estender a uma campanha com vários pisos: atribuímos a cada sala um índice de piso e permitimos que arestas de escada ou elevador conectem vértices em pisos diferentes. Cada piso tem sua própria realização geométrica $\varphi_f$, mas a conectividade e a progressão são verificadas no grafo conjunto. Essa extensão ainda não está implementada neste crate.

## 11. Parâmetros do modelo

Os parâmetros podem ser agrupados por função. Os nomes entre parênteses são os usados no crate quando há correspondência direta.

| Grupo | Parâmetros | Efeito |
|---|---|---|
| Andaime | colunas, linhas, tamanho de célula, folga, blocos grandes, fração descartada, irregularidade e rotação (`cols`, `rows`, `cellW`, `cellH`, `gap`, `big`, `drop`, `layoutJitter`, `rotateRooms`) | escala, fragmentação e orientação das salas; a irregularidade é limitada para preservar o tamanho mínimo |
| Formas | pesos e cotas por classe, tamanho mínimo para estrela, homofilia (`weights`, `maxOf`, `starCells`, `homophily`) | variedade e distribuição das salas |
| Portas | direções permitidas, quantidade de portas em discos, passo de amostragem (`m`, `discPorts`, `portStep`) | posições e direções das conexões |
| Corredores | largura, afastamento, trecho de saída, segmento mínimo, distância mínima, máximo de dobras (`width`, `clear`, `exit`, `seg`, `minLen`, `maxBends`) | geometria das passagens |
| Pesos | probabilidades por tipo de rota, inclusive desvio W, custos de dobra/comprimento/diagonal (`pI`, `pL`, `pZ`, `pW`, `pT`, `pX`, `costBend`, `costLen`, `cost45`, `costFacing`) | preferência entre candidatos válidos |
| Topologia | proximidade, raio, densidade e perfil (`proximity`, `near`, `delta`, `topology`) | pares considerados, alvo de ciclos e requisitos da rede |
| Progressão | quantidade de travas com chave e interruptor (`progression`) | condições de passagem verificadas no grafo de estados |
| Ritmo | papéis distribuídos no caminho principal (`pacing`) | contraste entre avanço, descanso, combate e exploração opcional |
| Início e saída | limite de portas das escadas e distância mínima em saltos (`stairsDoors`, `minHops`) | posição topológica das escadas |
| Conteúdo | quantidade de emboscadas e câmaras-folha; classes e papéis permitidos (`ambush`, `leaves`, `leafClasses`, `leafRoles`) | distribuição de funções existentes |
| Mistura | matriz de compatibilidade entre classes (`mix`) | permite ou desestimula conexões entre formas |
| Junções | condutância opcional (`junction`) | parâmetro de referência, ainda não usado pelo gerador |

O perfil $\Pi$ complementa $\Lambda$: os parâmetros controlam como o gerador opera; o perfil descreve a estrutura que se deseja obter.

## 12. O que existe no crate Rust

Esta tabela evita confundir a especificação de referência com a implementação deste repositório.

| Conceito | Situação no crate |
|---|---|
| Salas e geometrias | Retângulos, discos, octógonos, formas L e estrelas; posicionamento baseado numa grade de blocos. |
| Direções e rotas | Modos com 4 ou 8 direções; candidatas I/L/Z, ramificações T e cruzamentos X. |
| Conexão principal | Crescimento de árvore sobre pares de salas candidatos; o gerador tenta outras relações de proximidade se ficar sem conexões. |
| Ciclos | Escolha posterior de ações de ciclo até o alvo derivado de `delta` e da referência `betaMax`. |
| Início e escadas | Um início e uma sala de escadas por piso; as escadas são escolhidas entre salas distantes, normalmente folhas. |
| Conteúdo de sala | Marcação de início, escada, chefe, descanso, emboscada, armadilha, prêmio, chave, interruptor ou vazio. Os rótulos não constituem um sistema de missões. |
| Proximidade | O enum Rust inclui RNG, Gabriel e Near por raio. Delaunay aparece na especificação de referência, mas não no enum atual. |
| Perfis topológicos | Requisitos de ramos, profundidade, rotas independentes, hub e ciclo mínimo implementados; pontuação global de perfis ainda não implementada. |
| Progressão com estado | Chaves, interruptores, portas condicionais e verificação de alcançabilidade implementados; objetivos de missão ainda não implementados. |
| Ritmo de exploração | Papéis de descanso, chefe e emboscada no caminho principal e maior preferência por prêmios nos ramos opcionais. |
| Flexibilidade espacial | Grade irregular limitada por dimensões mínimas, rotação de formas e rotas W de até seis dobras. |
| Vários pisos conectados | Cada geração atual produz um único mapa 2D; relação de progressão entre pisos não está modelada aqui. |

No crate, as verificações geométricas e topológicas estão distribuídas principalmente por [generate.rs](../../crates/tapiti-core/src/generate.rs), [validate.rs](../../crates/tapiti-core/src/validate.rs) e [metrics.rs](../../crates/tapiti-core/src/metrics.rs). O validador é a referência para determinar se uma saída concreta satisfaz as condições codificadas.

## 13. Resumo

O modelo separa três perguntas:

1. **A geometria cabe?** Salas, portas e corredores respeitam larguras e folgas?
2. **A rede tem a forma desejada?** Os ciclos, ramos, gargalos e rotas até a saída correspondem ao perfil?
3. **O nível pode ser concluído?** Se houver regras de progressão, a saída continua alcançável no estado correto?

Essa separação mantém a matemática clara. A geometria define o que é fisicamente possível; o grafo define a estrutura de navegação; os perfis definem a intenção do nível. O próximo avanço natural é expressar a intenção como condições sobre $H$, sem confundir a quantidade global de ciclos com a disponibilidade de rotas alternativas em pontos específicos.
