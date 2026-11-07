#[derive(Clone, Debug)]
pub enum FieldKind {
    Number { min: f64, max: f64, integer: bool },
    Boolean,
    Choice(Vec<(String, String)>),
}

#[derive(Clone, Debug)]
pub struct ParamField {
    pub path: String,
    pub label: String,
    pub help: String,
    pub kind: FieldKind,
}

#[derive(Clone, Debug)]
pub struct ParamGroup {
    pub id: String,
    pub title: String,
    pub intro: String,
    pub fields: Vec<ParamField>,
    pub open_by_default: bool,
}

fn number(path: &str, label: &str, min: f64, max: f64, integer: bool) -> ParamField {
    ParamField {
        path: path.into(),
        label: label.into(),
        help: help(path, label),
        kind: FieldKind::Number { min, max, integer },
    }
}

fn boolean(path: &str, label: &str) -> ParamField {
    ParamField {
        path: path.into(),
        label: label.into(),
        help: help(path, label),
        kind: FieldKind::Boolean,
    }
}

fn choice(path: &str, label: &str, options: &[(&str, &str)]) -> ParamField {
    ParamField {
        path: path.into(),
        label: label.into(),
        help: help(path, label),
        kind: FieldKind::Choice(
            options
                .iter()
                .map(|(value, name)| ((*value).into(), (*name).into()))
                .collect(),
        ),
    }
}

fn group(id: &str, title: &str, intro: &str, fields: Vec<ParamField>) -> ParamGroup {
    ParamGroup {
        id: id.into(),
        title: title.into(),
        intro: intro.into(),
        fields,
        open_by_default: matches!(id, "shapes" | "graph"),
    }
}

fn class_fields(prefix: &str, title: &str, range: (f64, f64), integer: bool) -> Vec<ParamField> {
    [
        ("rect", "retangulares"),
        ("disc", "circulares"),
        ("oct", "octogonais"),
        ("L", "em L"),
        ("star", "estrelas truncadas"),
    ]
    .into_iter()
    .map(|(class, label)| {
        number(
            &format!("{prefix}.{class}"),
            &format!("{title} · {label}"),
            range.0,
            range.1,
            integer,
        )
    })
    .collect()
}

pub fn groups() -> Vec<ParamGroup> {
    let mut shapes = class_fields("weights", "Peso", (0.0, 1000.0), false);
    shapes.extend(class_fields("maxOf", "Máximo", (0.0, 10000.0), true));
    shapes.extend([
        number(
            "starCells",
            "Células mínimas para estrela",
            1.0,
            1000.0,
            true,
        ),
        number(
            "homophily",
            "Preferência por formas vizinhas",
            -1.0,
            1.0,
            false,
        ),
    ]);

    let mut corridors = vec![
        number("width", "Largura do corredor (px)", 0.1, 10000.0, false),
        number("clear", "Folga σ (px)", 0.0, 1000.0, false),
        number("exit", "Trecho de saída (px)", 0.1, 10000.0, false),
        number("seg", "Segmento mínimo (px)", 0.1, 10000.0, false),
        number(
            "minLen",
            "Distância mínima entre salas (px)",
            0.1,
            10000.0,
            false,
        ),
        choice(
            "maxBends",
            "Dobras máximas",
            &[
                ("0", "0"),
                ("1", "1"),
                ("2", "2"),
                ("4", "4"),
                ("5", "5"),
                ("6", "6"),
            ],
        ),
    ];
    corridors.extend(
        [
            ("pI", "Peso · reto I"),
            ("pL", "Peso · uma dobra L"),
            ("pZ", "Peso · duas dobras Z/U"),
            ("pW", "Peso · desvio W"),
            ("pT", "Peso · junção T"),
            ("pX", "Peso · cruzamento X"),
            ("costBend", "Custo por dobra"),
            ("costLen", "Custo por pixel"),
            ("cost45", "Custo por dobra de 45°"),
            ("costFacing", "Bônus por portas frente a frente"),
        ]
        .into_iter()
        .map(|(path, label)| number(path, label, 0.0, 1000.0, false)),
    );

    let classes = [
        ("rect", "retangular"),
        ("disc", "circular"),
        ("oct", "octogonal"),
        ("L", "em L"),
        ("star", "estrela truncada"),
    ];
    let mut mix = Vec::new();
    for (i, (a, a_label)) in classes.iter().enumerate() {
        for (b, b_label) in classes.iter().skip(i) {
            mix.push(number(
                &format!("mix.{a}-{b}"),
                &format!("{a_label} ↔ {b_label}"),
                0.0,
                1.0,
                false,
            ));
        }
    }

    let mut leaves = vec![number("leaves", "Quantidade de câmaras", 0.0, 1000.0, true)];
    leaves.extend(class_fields(
        "leafClasses",
        "Peso · sala",
        (0.0, 1000.0),
        false,
    ));
    leaves.extend([
        number(
            "leafRoles.trap",
            "Chance relativa · armadilha",
            0.0,
            1000.0,
            false,
        ),
        number(
            "leafRoles.prize",
            "Chance relativa · prêmio",
            0.0,
            1000.0,
            false,
        ),
        number(
            "leafRoles.empty",
            "Chance relativa · vazia",
            0.0,
            1000.0,
            false,
        ),
    ]);

    vec![
        group(
            "scaffold",
            "Grade e tamanho",
            "Define o espaço disponível e quantos blocos formam o andar.",
            vec![
                number("cols", "Colunas", 1.0, 64.0, true),
                number("rows", "Linhas", 1.0, 64.0, true),
                number("cellW", "Largura da célula (px)", 1.0, 5000.0, false),
                number("cellH", "Altura da célula (px)", 1.0, 5000.0, false),
                number("gap", "Recuo entre salas (px)", 0.0, 1000.0, false),
                number("big", "Blocos grandes", 0.0, 1000.0, true),
                number("drop", "Fração de células livres", 0.0, 1.0, false),
                number("layoutJitter", "Irregularidade da grade", 0.0, 0.45, false),
                boolean("rotateRooms", "Girar formas poligonais"),
            ],
        ),
        group(
            "shapes",
            "Formas das salas",
            "Pesos maiores tornam uma forma mais provável; limites controlam a quantidade por tipo.",
            shapes,
        ),
        group(
            "directions",
            "Direções e portas",
            "Controla as direções permitidas para corredores e a amostragem de portas.",
            vec![
                choice("m", "Alfabeto de direções", &[("4", "4 direções"), ("8", "8 direções")]),
                choice(
                    "discPorts",
                    "Portas por sala circular",
                    &[("4", "4"), ("8", "8"), ("16", "16")],
                ),
                number("portStep", "Passo entre portas (px)", 0.1, 10000.0, false),
            ],
        ),
        group(
            "corridors",
            "Corredores e junções",
            "Medidas em pixels. Pesos favorecem tipos de caminho; custos mudam a escolha entre rotas válidas.",
            corridors,
        ),
        group(
            "mix",
            "Compatibilidade entre formas",
            "Cada valor multiplica a chance de conectar dois tipos de sala. Zero proíbe a ligação; um mantém a chance normal.",
            mix,
        ),
        group(
            "graph",
            "Conexões e densidade",
            "Escolhe pares de salas candidatos e quantos ciclos o gerador tenta criar.",
            vec![
                choice(
                    "proximity",
                    "Como escolher pares",
                    &[("rng", "RNG"), ("gabriel", "Gabriel"), ("near", "Perto")],
                ),
                number("near", "Raio entre centros (px)", 0.1, 10000.0, false),
                number("delta", "Densidade δ", 0.0, 1.0, false),
            ],
        ),
        group(
            "topology",
            "Perfil de topologia",
            "Escolha um padrão de conectividade ou ajuste seus requisitos; o gerador tenta satisfazê-los sem quebrar as regras geométricas.",
            vec![
                choice(
                    "topology.profile",
                    "Perfil",
                    &[
                        ("classic", "Clássico"),
                        ("backbone", "Espinha dorsal"),
                        ("hub", "Hub"),
                        ("redundantExit", "Saída redundante"),
                        ("longLoop", "Ciclo longo"),
                    ],
                ),
                number("topology.minBranches", "Ramos laterais mínimos", 0.0, 1000.0, true),
                number("topology.minBranchDepth", "Profundidade mínima dos ramos", 0.0, 1000.0, true),
                number("topology.minExitRoutes", "Rotas independentes até a escada", 0.0, 1000.0, true),
                number("topology.minCycleLength", "Comprimento mínimo do ciclo", 0.0, 1000.0, true),
                number("topology.minHubDegree", "Grau mínimo do hub", 0.0, 1000.0, true),
            ],
        ),
        group(
            "progression",
            "Progressão e ritmo",
            "Adicione portões que exigem itens ou interruptores e distribua papéis pelo caminho principal.",
            vec![
                number("progression.keyLocks", "Travas com chave", 0.0, 8.0, true),
                number("progression.switchLocks", "Travas com interruptor", 0.0, 8.0, true),
                boolean("pacing.enabled", "Ativar ritmo de exploração"),
            ],
        ),
        group(
            "growth",
            "Crescimento (protótipo)",
            "Em vez de uma sala por bloco da grade, as salas crescem uma a partir da outra, em posições livres. A grade só define o tamanho do mundo.",
            vec![
                boolean("growth.enabled", "Ativar crescimento"),
                number("growth.rooms", "Salas (0 = como a grade)", 0.0, 2000.0, true),
                number("growth.branch", "Filhos por sala (1 a N)", 1.0, 16.0, true),
                number("growth.depth", "Profundidade máxima", 1.0, 200.0, true),
                number("growth.islands", "Ilhas", 1.0, 16.0, true),
                number("growth.cluster", "Agrupamento (λ)", -10.0, 10.0, false),
                number("growth.reach", "Alcance dos corredores (células)", 0.0, 20.0, false),
            ],
        ),
        group(
            "roles",
            "Entrada, escada e papéis",
            "Controla as portas da escada, a distância da entrada e a quantidade de emboscadas.",
            vec![
                number("stairsDoors", "Máximo de portas na escada", 1.0, 1000.0, true),
                number("minHops", "Passos mínimos até a escada", 1.0, 1000.0, true),
                number("ambush", "Salas de emboscada", 0.0, 1000.0, true),
            ],
        ),
        group(
            "leaves",
            "Câmaras isoladas",
            "Salas extras ligadas por um único corredor, como becos com prêmio ou armadilha.",
            leaves,
        ),
    ]
}

pub fn field(path: &str) -> Option<ParamField> {
    groups()
        .into_iter()
        .flat_map(|group| group.fields)
        .find(|field| field.path == path)
}

fn help(path: &str, label: &str) -> String {
    match path {
        "cols" => "Número de colunas. Mais colunas aumentam a largura potencial do andar.".into(),
        "rows" => "Número de linhas. Mais linhas aumentam a altura potencial do andar.".into(),
        "cellW" => "Largura de cada célula da grade. Blocos grandes podem ocupar várias células.".into(),
        "cellH" => "Altura de cada célula da grade. Blocos grandes podem ocupar várias células.".into(),
        "gap" => "Recuo aplicado às salas dentro dos blocos para manter espaço entre vizinhas.".into(),
        "big" => "Quantidade máxima de blocos grandes que o gerador tenta colocar.".into(),
        "drop" => "Fração de células livres reservadas para possíveis câmaras-folha.".into(),
        "layoutJitter" => "Desloca linhas internas da grade. O gerador reduz o valor se faltar espaço para uma sala mínima.".into(),
        "rotateRooms" => "Permite girar formas poligonais em incrementos de 90° ou 45° no modo octilinear.".into(),
        "homophily" => "Valor positivo aproxima salas da mesma forma; negativo favorece formas diferentes; zero ignora as vizinhas.".into(),
        "m" => "4 permite ângulos de 90°. 8 também permite diagonais de 45°.".into(),
        "discPorts" => "Número de direções possíveis na borda circular. Direções fora do alfabeto ativo são descartadas.".into(),
        "portStep" => "Distância entre posições de porta tentadas. Um passo menor tenta mais posições e aumenta o custo.".into(),
        "width" => "Largura física de cada corredor.".into(),
        "clear" => "Distância mínima entre corredores e salas ou corredores que não se conectam.".into(),
        "exit" => "Comprimento mínimo que o corredor percorre após sair da porta antes de virar.".into(),
        "seg" => "Comprimento mínimo de cada segmento interno do corredor.".into(),
        "minLen" => "Exige que o caminho e a distância direta entre salas conectadas atinjam este comprimento.".into(),
        "maxBends" => "Número máximo de dobras. Valores de 4 a 6 permitem desvios W mais sinuosos.".into(),
        "pI" => "Peso relativo de uma ligação reta, sem dobras.".into(),
        "pL" => "Peso relativo de uma ligação com uma dobra.".into(),
        "pZ" => "Peso relativo de caminhos com duas dobras, em Z ou U.".into(),
        "pW" => "Peso de desvios W; só há alternativas quando pelo menos quatro dobras são permitidas.".into(),
        "pT" => "Peso relativo para ligar uma sala a um corredor existente e formar uma junção T.".into(),
        "pX" => "Peso relativo para cruzar um corredor existente e formar uma junção X.".into(),
        "costBend" => "Penalidade somada a cada dobra. Um valor maior favorece caminhos retos.".into(),
        "costLen" => "Penalidade multiplicada pelo comprimento do caminho em pixels.".into(),
        "cost45" => "Penalidade adicional para cada dobra diagonal de 45°.".into(),
        "costFacing" => "Desconto quando as portas ficam frente a frente e permitem ligação reta.".into(),
        "proximity" => "RNG e Gabriel filtram pares por proximidade geométrica. Perto usa um raio máximo.".into(),
        "near" => "Raio máximo entre centros de salas no modo Perto; ignorado nos outros modos.".into(),
        "delta" => "Fração da referência de ciclos que o gerador tenta alcançar. Valores maiores favorecem atalhos e rotas alternativas.".into(),
        "stairsDoors" => "Número máximo de corredores que podem entrar na sala da escada.".into(),
        "minHops" => "Número mínimo de ligações entre a entrada e a escada, contado em salas.".into(),
        "ambush" => "Quantidade de salas comuns maiores que recebem o papel de emboscada.".into(),
        "topology.profile" => "Clássico mantém o comportamento base; outros perfis exigem ramos, hub, rotas redundantes ou ciclos longos.".into(),
        "topology.minBranches" => "Número mínimo de componentes laterais ligados ao caminho principal por uma passagem.".into(),
        "topology.minBranchDepth" => "Número mínimo de salas em um ramo a partir do caminho principal.".into(),
        "topology.minExitRoutes" => "Número mínimo de caminhos independentes entre entrada e escada.".into(),
        "topology.minCycleLength" => "Número mínimo de trechos no menor ciclo. Zero não impõe esse requisito.".into(),
        "topology.minHubDegree" => "Número mínimo de braços em um ponto central. Zero não exige um hub.".into(),
        "progression.keyLocks" => "Quantidade de portões que exigem uma chave coletada antes de atravessar.".into(),
        "progression.switchLocks" => "Quantidade de portões que exigem ativar um interruptor antes de atravessar.".into(),
        "growth.enabled" => "Liga o gerador por crescimento. Câmaras isoladas não são geradas neste modo.".into(),
        "growth.rooms" => "Quantidade de salas a crescer. Zero usa o número de blocos vivos que a grade daria.".into(),
        "growth.branch" => "Cada sala gera de 1 até este número de filhos antes de o sorteio escolher outro pai.".into(),
        "growth.depth" => "Geração mais funda que uma sala pode ter; a primeira sala é a 0.".into(),
        "growth.islands" => "Divide as salas em ilhas ligadas por pontes longas. Com mais de uma, os ciclos ficam dentro de cada ilha.".into(),
        "growth.cluster" => "Positivo concentra novas salas onde já há muitas; negativo espalha; zero é neutro.".into(),
        "growth.reach" => "Distância média dos corredores além do mínimo, em células da grade.".into(),
        "pacing.enabled" => "Distribui descanso, chefe e emboscadas pelo caminho principal e prêmios em ramos opcionais.".into(),
        "leaves" => "Quantidade de câmaras extras que o gerador tenta colocar em células livres.".into(),
        "leafRoles.trap" => "Peso relativo para uma câmara-folha receber uma armadilha.".into(),
        "leafRoles.prize" => "Peso relativo para uma câmara-folha receber um prêmio.".into(),
        "leafRoles.empty" => "Peso relativo para uma câmara-folha ficar vazia.".into(),
        _ if path.starts_with("weights.") => format!("Peso de sorteio para salas {label}. É relativo aos outros pesos e não precisa somar 1."),
        _ if path.starts_with("maxOf.") => format!("Limite de salas {label}. Se a soma dos limites for baixa demais, o gerador pode excedê-los."),
        _ if path.starts_with("leafClasses.") => format!("Peso relativo para escolher salas {label} como câmaras-folha."),
        _ if path.starts_with("mix.") => "Compatibilidade entre duas formas: zero proíbe a ligação; um mantém a chance normal.".into(),
        _ if path.starts_with("leafRoles.") => format!("Peso relativo para a câmara-folha receber o papel {label}."),
        _ => format!("{label}. Ajuste este valor e observe como ele altera a configuração do andar."),
    }
}
