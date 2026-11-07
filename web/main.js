import init, { default_params, stage_json, bench } from './pkg/dungeon_lab.js'
import { createPool } from './pool.js'

await init()
const pool = createPool()

const CLASSES = ['rect', 'disc', 'oct', 'L', 'star']
const CLASS_LABELS = { rect: 'retangular', disc: 'circular', oct: 'octogonal', L: 'em L', star: 'estrela truncada' }
const COLORS = { rect: '#496c9a', disc: '#29a89a', oct: '#9b78d1', L: '#ce843c', star: '#d0ae38' }
const ROLE_COLORS = { start: '#62d99a', stairs: '#dc78dc', ambush: '#fb7777', boss: '#ff827d', rest: '#6fc8e8', key: '#ffca58', switch: '#7bb9ff', trap: '#f26c6c', prize: '#f3c45d', empty: '#a7b1c0' }
const ROLE_LABELS = { start: 'entrada', stairs: 'escada', ambush: 'emboscada', boss: 'chefe', rest: 'descanso', key: 'chave', switch: 'interruptor', trap: 'armadilha', prize: 'prêmio', empty: 'vazia', plain: 'comum' }
const DIRS = [[1, 0], [Math.SQRT1_2, Math.SQRT1_2], [0, 1], [-Math.SQRT1_2, Math.SQRT1_2], [-1, 0], [-Math.SQRT1_2, -Math.SQRT1_2], [0, -1], [Math.SQRT1_2, -Math.SQRT1_2]]
const LEVEL_INFO = [
  'Introdução: salas retangulares e poucas conexões extras.',
  'Acrescenta salas em L e as primeiras junções.',
  'Amplia o mapa e libera caminhos diagonais e salas circulares.',
  'Combina mais formas, junções e portas na escada.',
  'Configuração mais rica: mais salas, formas e alternativas de conexão.',
]

const state = { level: 1, seed: 1, P: JSON.parse(default_params(1)), defaults: JSON.parse(default_params(1)) }
const overlays = { cells: true, labels: true, ports: false, pairs: false, candidates: false }
const view = { zoom: 1, x: 0, y: 0 }
let current = null
let regenTimer = 0
let activeHelp = null
let helpId = 0

const h = (tag, attrs = {}, ...kids) => {
  const el = document.createElement(tag)
  for (const [key, value] of Object.entries(attrs)) el.setAttribute(key, value)
  el.append(...kids)
  return el
}
const clone = (value) => JSON.parse(JSON.stringify(value))
const formatNumber = (n, digits = 1) => Number(n).toLocaleString('pt-BR', { maximumFractionDigits: digits })

/* ---------------- Ajuda acessível ---------------- */

const helpTooltip = h('div', { id: 'help-tooltip', class: 'help-tooltip', role: 'tooltip', hidden: '' })
document.body.append(helpTooltip)

const hideHelp = () => {
  if (activeHelp) activeHelp.setAttribute('aria-expanded', 'false')
  activeHelp = null
  helpTooltip.hidden = true
  helpTooltip.classList.remove('visible')
}

const showHelp = (button, text, pinned = false) => {
  if (activeHelp && activeHelp !== button) activeHelp.setAttribute('aria-expanded', 'false')
  activeHelp = button
  helpTooltip.textContent = text
  helpTooltip.hidden = false
  helpTooltip.classList.add('visible')
  button.setAttribute('aria-expanded', pinned ? 'true' : 'false')
  const rect = button.getBoundingClientRect()
  const width = Math.min(320, window.innerWidth - 24)
  helpTooltip.style.width = `${width}px`
  const left = Math.max(12, Math.min(rect.left, window.innerWidth - width - 12))
  const top = rect.bottom + 8 + helpTooltip.offsetHeight < window.innerHeight ? rect.bottom + 8 : Math.max(12, rect.top - helpTooltip.offsetHeight - 8)
  helpTooltip.style.left = `${left}px`
  helpTooltip.style.top = `${top}px`
}

const help = (text) => {
  const button = h('button', { type: 'button', class: 'help-button', 'aria-label': `Ajuda: ${text}`, 'aria-expanded': 'false', 'aria-describedby': 'help-tooltip' }, '?')
  button.addEventListener('mouseenter', () => showHelp(button, text))
  button.addEventListener('focus', () => showHelp(button, text))
  button.addEventListener('mouseleave', () => { if (activeHelp === button && document.activeElement !== button) hideHelp() })
  button.addEventListener('blur', () => { if (activeHelp === button && button.getAttribute('aria-expanded') !== 'true') hideHelp() })
  button.addEventListener('click', (event) => {
    event.preventDefault()
    event.stopPropagation()
    if (activeHelp === button && button.getAttribute('aria-expanded') === 'true') hideHelp()
    else showHelp(button, text, true)
  })
  return button
}

document.addEventListener('click', (event) => {
  if (!event.target.closest('.help-button')) hideHelp()
})
window.addEventListener('scroll', hideHelp, true)
window.addEventListener('resize', hideHelp)

/* ---------------- Parâmetros ---------------- */

const mixKey = (a, b) => (CLASSES.indexOf(a) <= CLASSES.indexOf(b) ? `${a}-${b}` : `${b}-${a}`)

const GROUPS = [
  {
    id: 'scaffold', title: 'Grade e tamanho', open: false,
    intro: 'Define o espaço disponível e quantos blocos formam o andar.',
    fields: [['cols', 'Colunas'], ['rows', 'Linhas'], ['cellW', 'Largura da célula (px)'], ['cellH', 'Altura da célula (px)'], ['gap', 'Recuo entre salas g (px)'], ['big', 'Blocos grandes'], ['drop', 'Fração de células livres'], ['layoutJitter', 'Irregularidade da grade'], ['rotateRooms', 'Girar formas poligonais']],
  },
  {
    id: 'shapes', title: 'Formas das salas', open: true,
    intro: 'Pesos maiores tornam uma forma mais provável. Os limites controlam a quantidade máxima de cada tipo.',
    fields: [...CLASSES.map((c) => [`weights.${c}`, `Peso · ${CLASS_LABELS[c]}`]), ...CLASSES.map((c) => [`maxOf.${c}`, `Máximo · ${CLASS_LABELS[c]}`]), ['starCells', 'Células mínimas para estrela'], ['homophily', 'Preferência por formas vizinhas']],
  },
  {
    id: 'directions', title: 'Direções e portas', open: false,
    intro: 'Controla as direções permitidas para corredores e a amostragem das portas.',
    fields: [['m', 'Alfabeto de direções'], ['discPorts', 'Portas por sala circular'], ['portStep', 'Passo entre portas (px)']],
  },
  {
    id: 'corridors', title: 'Corredores e junções', open: false,
    intro: 'As medidas estão em pixels. Pesos favorecem um tipo de ligação; custos alteram a preferência entre caminhos válidos.',
    fields: [['width', 'Largura w (px)'], ['clear', 'Folga σ (px)'], ['exit', 'Trecho de saída (px)'], ['seg', 'Segmento mínimo (px)'], ['minLen', 'Distância mínima entre salas (px)'], ['maxBends', 'Dobras máximas'], ['pI', 'Peso · reto I'], ['pL', 'Peso · uma dobra L'], ['pZ', 'Peso · duas dobras Z/U'], ['pW', 'Peso · desvio W'], ['pT', 'Chance relativa · junção T'], ['pX', 'Chance relativa · cruzamento X'], ['costBend', 'Custo por dobra'], ['costLen', 'Custo por pixel'], ['cost45', 'Custo por dobra de 45°'], ['costFacing', 'Bônus por portas frente a frente']],
  },
  {
    id: 'mix', title: 'Compatibilidade entre formas', open: false,
    intro: 'Cada valor multiplica a chance de conectar os dois tipos de sala. Zero proíbe a conexão; um não altera a chance.',
    fields: CLASSES.flatMap((a, i) => CLASSES.slice(i).map((b) => [`mix.${mixKey(a, b)}`, `${CLASS_LABELS[a]} ↔ ${CLASS_LABELS[b]}`])),
  },
  {
    id: 'graph', title: 'Conexões e densidade', open: true,
    intro: 'Escolhe quais pares de salas podem ser considerados e quantos ciclos o gerador tenta criar.',
    fields: [['proximity', 'Como escolher pares'], ['near', 'Raio máximo entre centros (px)'], ['delta', 'Densidade δ']],
  },
  {
    id: 'topology', title: 'Perfil de topologia', open: false,
    intro: 'Escolha um padrão de conectividade ou ajuste seus requisitos. O gerador tenta satisfazê-los sem quebrar as regras geométricas.',
    fields: [['topology.profile', 'Perfil'], ['topology.minBranches', 'Ramos laterais mínimos'], ['topology.minBranchDepth', 'Profundidade mínima dos ramos'], ['topology.minExitRoutes', 'Rotas independentes até a escada'], ['topology.minCycleLength', 'Comprimento mínimo do ciclo'], ['topology.minHubDegree', 'Grau mínimo de um ponto central']],
  },
  {
    id: 'progression', title: 'Progressão e ritmo', open: false,
    intro: 'Adicione portas que exigem itens ou interruptores e distribua papéis pelo caminho principal.',
    fields: [['progression.keyLocks', 'Travas com chave'], ['progression.switchLocks', 'Travas com interruptor'], ['pacing.enabled', 'Ativar ritmo de exploração']],
  },
  {
    id: 'roles', title: 'Entrada, escada e papéis', open: false,
    intro: 'Controla as portas da escada, a distância mínima entre entrada e escada e a quantidade de emboscadas.',
    fields: [['stairsDoors', 'Máximo de portas na escada'], ['minHops', 'Passos mínimos até a escada'], ['ambush', 'Salas de emboscada']],
  },
  {
    id: 'leaves', title: 'Câmaras isoladas', open: false,
    intro: 'Câmaras-folha são salas extras ligadas por um único corredor, como becos com prêmio ou armadilha.',
    fields: [['leaves', 'Quantidade de câmaras'], ...CLASSES.map((c) => [`leafClasses.${c}`, `Peso · ${CLASS_LABELS[c]}`]), ['leafRoles.trap', 'Chance relativa · armadilha'], ['leafRoles.prize', 'Chance relativa · prêmio'], ['leafRoles.empty', 'Chance relativa · vazia']],
  },
]

const PARAM_HELP = {
  cols: 'Quantas colunas de células formam a grade. Mais colunas aumentam a largura potencial do andar.',
  rows: 'Quantas linhas de células formam a grade. Mais linhas aumentam a altura potencial do andar.',
  cellW: 'Largura de cada célula da grade, em pixels. Blocos grandes podem ocupar várias células.',
  cellH: 'Altura de cada célula da grade, em pixels. Blocos grandes podem ocupar várias células.',
  gap: 'Recuo de cada bloco antes de desenhar a sala. Ajuda a manter espaço entre salas vizinhas.',
  big: 'Número máximo de blocos grandes que o gerador tenta posicionar antes de preencher o restante com salas menores.',
  drop: 'Fração das células pequenas que o gerador tenta deixar livres para futuras câmaras-folha. Ex.: 0,2 significa 20%.',
  layoutJitter: 'Desloca linhas internas da grade em até esta fração da largura ou altura da célula. As bordas externas e a ordem das linhas são preservadas; o gerador reduz o valor se faltar espaço para uma sala mínima.',
  rotateRooms: 'Permite girar formas poligonais em incrementos de 90° (ou 45° no modo octilinear), aumentando a variedade de orientações.',
  'weights.rect': 'Peso de sorteio para salas retangulares. É relativo aos pesos das outras formas; não precisa somar 1.',
  'weights.disc': 'Peso de sorteio para salas circulares. É relativo aos pesos das outras formas; não precisa somar 1.',
  'weights.oct': 'Peso de sorteio para salas octogonais. É relativo aos pesos das outras formas; não precisa somar 1.',
  'weights.L': 'Peso de sorteio para salas em L. É relativo aos pesos das outras formas; não precisa somar 1.',
  'weights.star': 'Peso de sorteio para estrelas truncadas. Só podem aparecer em blocos com células suficientes.',
  'maxOf.rect': 'Limite máximo de salas retangulares. Se a soma dos limites for menor que o total de salas, algum limite pode ser excedido.',
  'maxOf.disc': 'Limite máximo de salas circulares. Se a soma dos limites for menor que o total de salas, algum limite pode ser excedido.',
  'maxOf.oct': 'Limite máximo de salas octogonais. Se a soma dos limites for menor que o total de salas, algum limite pode ser excedido.',
  'maxOf.L': 'Limite máximo de salas em L. Se a soma dos limites for menor que o total de salas, algum limite pode ser excedido.',
  'maxOf.star': 'Limite máximo de estrelas truncadas. Se a soma dos limites for menor que o total de salas, algum limite pode ser excedido.',
  starCells: 'Número mínimo de células do bloco para permitir uma estrela truncada.',
  homophily: 'Controla a influência das formas vizinhas: valor positivo aproxima salas iguais; negativo favorece salas diferentes; zero ignora vizinhos.',
  m: '4 permite corredores retos em ângulos de 90°. 8 também permite diagonais de 45°.',
  discPorts: 'Quantidade de direções possíveis na borda de salas circulares. Direções fora do alfabeto atual são descartadas.',
  portStep: 'Distância entre posições de porta amostradas nas bordas poligonais. Um passo menor tenta mais posições, mas aumenta o custo de geração.',
  width: 'Largura física de cada corredor.',
  clear: 'Distância mínima entre corredores e salas ou corredores que não se conectam.',
  exit: 'Comprimento mínimo do corredor ao sair de uma porta antes de poder virar.',
  seg: 'Comprimento mínimo de cada segmento interno do corredor.',
  minLen: 'Exige que o caminho e a distância em linha reta entre salas conectadas atinjam este comprimento.',
  maxBends: 'Quantidade máxima de dobras em uma ligação entre duas portas: 0 reta, 1 em L, 2 em Z/U. Valores de 4 a 6 permitem desvios W mais sinuosos.',
  pI: 'Peso relativo de uma ligação reta, sem dobras.',
  pL: 'Peso relativo de uma ligação com uma dobra.',
  pZ: 'Peso relativo de uma ligação com duas dobras.',
  pW: 'Peso relativo de um caminho com desvio adicional em forma de W. Só gera alternativas quando há ao menos quatro dobras disponíveis.',
  pT: 'Peso relativo para ligar uma sala a um corredor existente, formando uma junção T.',
  pX: 'Peso relativo para cruzar um corredor existente e formar uma junção X.',
  costBend: 'Penalidade somada a cada dobra. Um custo maior favorece caminhos mais retos.',
  costLen: 'Penalidade multiplicada pelo comprimento do caminho, em pixels.',
  cost45: 'Penalidade adicional por cada dobra diagonal de 45°.',
  costFacing: 'Desconto quando as portas ficam frente a frente e permitem uma ligação reta.',
  proximity: 'RNG e Gabriel filtram pares por proximidade geométrica. “Perto” usa um raio máximo entre centros.',
  near: 'Raio máximo entre os centros de duas salas no modo “Perto”. É ignorado nos outros modos.',
  delta: 'Fração da referência de ciclos que o gerador tenta alcançar. 0 tende a uma árvore; valores maiores favorecem atalhos e rotas alternativas.',
  stairsDoors: 'Número máximo de corredores que podem entrar na sala da escada.',
  minHops: 'Quantidade mínima de ligações entre a entrada e a escada, medida em salas percorridas.',
  ambush: 'Quantidade de salas comuns maiores que recebem o papel de emboscada.',
  'topology.profile': 'Padrão de conectividade: Clássico mantém o comportamento atual; Espinha dorsal exige ramos; Hub favorece uma junção central; Saída redundante exige caminhos sem trechos compartilhados; Ciclo longo exige um circuito maior.',
  'topology.minBranches': 'Quantidade mínima de componentes laterais ligados ao caminho principal por uma passagem.',
  'topology.minBranchDepth': 'Maior quantidade de salas que um ramo deve ter a partir do caminho principal.',
  'topology.minExitRoutes': 'Quantidade mínima de caminhos entre entrada e escada que não compartilham trechos de corredor.',
  'topology.minCycleLength': 'Número mínimo de trechos no menor ciclo do grafo. Use zero para não impor esse requisito.',
  'topology.minHubDegree': 'Número mínimo de braços em pelo menos um ponto do mapa. Use zero para não exigir um hub.',
  'progression.keyLocks': 'Quantidade de portões em que todos os caminhos adiante exigem uma chave recolhida antes do portão. O modelo aceita até oito chaves.',
  'progression.switchLocks': 'Quantidade de portões em que todos os caminhos adiante exigem ativar um interruptor antes do portão. O modelo aceita até oito interruptores.',
  'pacing.enabled': 'Distribui salas de descanso, chefe e emboscada pelo caminho principal e favorece prêmios em ramos opcionais.',
  leaves: 'Número de câmaras extras que o gerador tenta colocar em células livres.',
  'leafRoles.trap': 'Peso relativo para uma câmara-folha receber uma armadilha.',
  'leafRoles.prize': 'Peso relativo para uma câmara-folha receber um prêmio.',
  'leafRoles.empty': 'Peso relativo para uma câmara-folha ficar vazia.',
  junction: 'Condutância entre braços de uma junção. “∞” faz os braços se comportarem como um único espaço. Este parâmetro é informativo e ainda não altera a geração.',
}
for (const c of CLASSES) PARAM_HELP[`leafClasses.${c}`] = `Peso relativo para escolher salas ${CLASS_LABELS[c]} como câmaras-folha.`
for (const a of CLASSES) for (const b of CLASSES) {
  const path = `mix.${mixKey(a, b)}`
  PARAM_HELP[path] = `Compatibilidade entre salas ${CLASS_LABELS[a]} e ${CLASS_LABELS[b]}. Zero proíbe a ligação; um mantém a chance normal.`
}

const FIELD_OPTIONS = {
  m: [4, 8],
  discPorts: [4, 8, 16],
  maxBends: [0, 1, 2, 4, 5, 6],
  proximity: ['rng', 'gabriel', 'near'],
  'topology.profile': ['classic', 'backbone', 'hub', 'redundantExit', 'longLoop'],
}
const INTEGER_FIELDS = new Set(['cols', 'rows', 'big', 'starCells', 'discPorts', 'maxBends', 'stairsDoors', 'minHops', 'ambush', 'leaves', 'topology.minBranches', 'topology.minBranchDepth', 'topology.minExitRoutes', 'topology.minCycleLength', 'topology.minHubDegree', 'progression.keyLocks', 'progression.switchLocks', ...CLASSES.map((c) => `maxOf.${c}`)])
const BOOLEAN_FIELDS = new Set(['rotateRooms', 'pacing.enabled'])
const OPTION_LABELS = { classic: 'Clássico', backbone: 'Espinha dorsal', hub: 'Hub', redundantExit: 'Saída redundante', longLoop: 'Ciclo longo' }

const limitsFor = (path) => {
  if (path === 'cols' || path === 'rows') return { min: 1, max: 64 }
  if (path === 'cellW' || path === 'cellH') return { min: 1, max: 5000 }
  if (path === 'gap' || path === 'clear') return { min: 0, max: 1000 }
  if (path === 'drop' || path === 'delta') return { min: 0, max: 1 }
  if (path === 'layoutJitter') return { min: 0, max: 0.45 }
  if (path === 'homophily') return { min: -1, max: 1 }
  if (path === 'portStep' || path === 'width' || path === 'exit' || path === 'seg' || path === 'minLen' || path === 'near') return { min: 0.1, max: 10000 }
  if (path.startsWith('weights.') || path.startsWith('leafClasses.') || path.startsWith('leafRoles.')) return { min: 0, max: 1000 }
  if (path.startsWith('mix.')) return { min: 0, max: 1 }
  if (path.startsWith('p')) return { min: 0, max: 1000 }
  if (path.startsWith('cost')) return { min: 0, max: 1000 }
  if (path === 'big' || path === 'ambush' || path === 'leaves') return { min: 0, max: 1000 }
  if (path.startsWith('maxOf.')) return { min: 0, max: 10000 }
  if (path.startsWith('topology.')) return { min: 0, max: 1000 }
  if (path === 'progression.keyLocks' || path === 'progression.switchLocks') return { min: 0, max: 8 }
  if (path === 'starCells' || path === 'stairsDoors' || path === 'minHops') return { min: 1, max: 1000 }
  return { min: 0, max: 10000 }
}

const getPath = (object, path) => path.split('.').reduce((value, key) => value[key], object)
const setPath = (object, path, value) => {
  const keys = path.split('.')
  const last = keys.pop()
  keys.reduce((target, key) => target[key], object)[last] = value
}

const form = h('div', { class: 'parameter-groups' })
const changedCount = () => GROUPS.flatMap((g) => g.fields.map(([path]) => path)).filter((path) => getPath(state.P, path) !== getPath(state.defaults, path)).length
const updateChangedIndicator = () => {
  changedStatus.textContent = changedCount() === 0 ? 'Configuração padrão' : `${changedCount()} parâmetro(s) alterado(s)`
  changedStatus.classList.toggle('changed', changedCount() > 0)
}

const resetGroup = (group) => {
  for (const [path] of group.fields) setPath(state.P, path, clone(getPath(state.defaults, path)))
  buildForm()
  updateChangedIndicator()
  scheduleRegenerate()
}

const buildForm = () => {
  const expanded = new Set([...form.querySelectorAll('details[open]')].map((node) => node.dataset.group))
  form.replaceChildren()
  for (const group of GROUPS) {
    const box = h('details', { class: 'parameter-group', 'data-group': group.id })
    box.open = expanded.size ? expanded.has(group.id) : !!group.open
    const summary = h('summary', {}, h('span', {}, group.title), h('span', { class: 'group-count' }, `${group.fields.length}`))
    box.append(summary, h('p', { class: 'group-intro' }, group.intro))
    if (group.fields.length) {
      box.append(h('button', { type: 'button', class: 'text-button group-reset' }, 'Restaurar este grupo'))
      box.querySelector('.group-reset').addEventListener('click', () => resetGroup(group))
    }
    for (const [path, label] of group.fields) {
      const id = `param-${path.replaceAll('.', '-')}`
      const line = h('div', { class: 'field' })
      const options = FIELD_OPTIONS[path]
      const limits = limitsFor(path)
      const optionName = (option) => path === 'proximity'
        ? ({ rng: 'RNG', gabriel: 'Gabriel', near: 'Perto' }[option])
        : (OPTION_LABELS[option] ?? String(option))
      const allowed = BOOLEAN_FIELDS.has(path)
        ? 'Marque para ativar; desmarque para desativar.'
        : options
          ? `Opções: ${options.map(optionName).join(', ')}.`
          : `Faixa aceita: ${formatNumber(limits.min, 2)} a ${formatNumber(limits.max, 2)}${INTEGER_FIELDS.has(path) ? ', números inteiros' : ''}.`
      const helpText = `${PARAM_HELP[path] ?? `Parâmetro ${label}.`} ${allowed}`
      const title = h('div', { class: 'field-title' }, h('label', { for: id }, label), help(helpText))
      const error = h('span', { class: 'field-error', 'aria-live': 'polite' })
      let input
      if (BOOLEAN_FIELDS.has(path)) {
        input = h('input', { id, name: path, type: 'checkbox', 'aria-label': label })
        input.checked = !!getPath(state.P, path)
        input.addEventListener('change', () => {
          setPath(state.P, path, input.checked)
          updateChangedIndicator()
          scheduleRegenerate()
        })
      } else if (options) {
        input = h('select', { id, name: path })
        for (const option of options) input.append(h('option', { value: String(option) }, optionName(option)))
        input.value = String(getPath(state.P, path))
        input.addEventListener('change', () => {
          setPath(state.P, path, typeof getPath(state.P, path) === 'number' ? Number(input.value) : input.value)
          updateChangedIndicator()
          scheduleRegenerate()
        })
      } else {
        const limits = limitsFor(path)
        input = h('input', { id, name: path, type: 'number', value: String(getPath(state.P, path)), min: String(limits.min), max: String(limits.max), step: INTEGER_FIELDS.has(path) ? '1' : 'any', inputmode: 'decimal' })
        input.addEventListener('change', () => {
          const value = Number(input.value)
          const integerRequired = INTEGER_FIELDS.has(path)
          if (!input.value || !Number.isFinite(value) || value < limits.min || value > limits.max || (integerRequired && !Number.isInteger(value))) {
            error.textContent = integerRequired ? `Use um inteiro entre ${limits.min} e ${limits.max}.` : `Use um valor entre ${limits.min} e ${limits.max}.`
            input.setAttribute('aria-invalid', 'true')
            return
          }
          error.textContent = ''
          input.removeAttribute('aria-invalid')
          setPath(state.P, path, value)
          updateChangedIndicator()
          scheduleRegenerate()
        })
      }
      line.append(title, input, error)
      box.append(line)
    }
    form.append(box)
  }
  const junction = h('details', { class: 'parameter-group', 'data-group': 'junction-info' }, h('summary', {}, h('span', {}, 'Condutância das junções'), h('span', { class: 'group-count' }, 'info')))
  const junctionValue = state.P.junction == null ? '∞ (braços unidos)' : formatNumber(state.P.junction, 2)
  junction.append(h('p', { class: 'group-intro' }, PARAM_HELP.junction), h('div', { class: 'readonly-setting' }, h('span', {}, 'Valor atual'), h('strong', {}, junctionValue), help(PARAM_HELP.junction)))
  form.append(junction)
}

/* ---------------- Desenho do mapa ---------------- */

const canvas = h('canvas', { id: 'dungeon-map', tabindex: '0', role: 'img', 'aria-label': 'Mapa do andar. Use o mouse para explorar as salas e os corredores.' })
const ctx = canvas.getContext('2d')
const mapTooltip = h('div', { class: 'map-tooltip', role: 'tooltip', 'aria-live': 'polite', hidden: '' })
const zoomLabel = h('span', { class: 'zoom-label', 'aria-live': 'polite' }, '100%')
const viewSection = h('section', { id: 'view', 'aria-label': 'Visualização do mapa' }, canvas, mapTooltip)

const pathOf = (shape) => {
  ctx.beginPath()
  if (shape.kind === 'disc') ctx.arc(shape.c[0], shape.c[1], shape.r, 0, Math.PI * 2)
  else shape.pts.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)))
  ctx.closePath()
}

const transformOf = () => {
  const W = canvas.clientWidth
  const H = canvas.clientHeight
  const P = current?.dungeon.params
  if (!P) return { W, H, k: 1, ox: 0, oy: 0, worldW: 1, worldH: 1 }
  const worldW = P.cols * P.cellW
  const worldH = P.rows * P.cellH
  const fit = Math.min(Math.max(1, W - 48) / worldW, Math.max(1, H - 48) / worldH)
  const k = fit * view.zoom
  return { W, H, k, ox: W / 2 + view.x - (worldW / 2) * k, oy: H / 2 + view.y - (worldH / 2) * k, worldW, worldH }
}

const draw = () => {
  const dpr = window.devicePixelRatio || 1
  const W = canvas.clientWidth
  const H = canvas.clientHeight
  if (!W || !H) return
  if (canvas.width !== Math.round(W * dpr) || canvas.height !== Math.round(H * dpr)) {
    canvas.width = Math.round(W * dpr)
    canvas.height = Math.round(H * dpr)
  }
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.clearRect(0, 0, W, H)
  if (!current) return
  const { W: width, H: height, k, ox, oy, worldW, worldH } = transformOf()
  ctx.setTransform(dpr * k, 0, 0, dpr * k, dpr * ox, dpr * oy)
  const px = 1 / k
  const d = current.dungeon
  const P = d.params

  if (overlays.cells) {
    ctx.strokeStyle = 'rgba(153, 181, 215, .12)'
    ctx.lineWidth = px
    const gridX = d.gridX?.length === P.cols + 1 ? d.gridX : Array.from({ length: P.cols + 1 }, (_, i) => i * P.cellW)
    const gridY = d.gridY?.length === P.rows + 1 ? d.gridY : Array.from({ length: P.rows + 1 }, (_, i) => i * P.cellH)
    for (const x of gridX) (ctx.beginPath(), ctx.moveTo(x, gridY[0]), ctx.lineTo(x, gridY.at(-1)), ctx.stroke())
    for (const y of gridY) (ctx.beginPath(), ctx.moveTo(gridX[0], y), ctx.lineTo(gridX.at(-1), y), ctx.stroke())
  }
  if (overlays.pairs) {
    ctx.strokeStyle = 'rgba(120, 170, 235, .48)'
    ctx.setLineDash([7 * px, 6 * px])
    ctx.lineWidth = px
    for (const [a, b] of d.pairs) {
      ctx.beginPath()
      ctx.moveTo(...d.rooms[a].center)
      ctx.lineTo(...d.rooms[b].center)
      ctx.stroke()
    }
    ctx.setLineDash([])
  }
  if (overlays.candidates) {
    ctx.strokeStyle = 'rgba(94, 191, 255, .42)'
    ctx.lineWidth = 2 * px
    for (const candidate of d.candidates) {
      ctx.beginPath()
      candidate.pts.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)))
      ctx.stroke()
    }
  }
  const hasCorridorRegions = d.edges.every((edge) => Array.isArray(edge.region))
  if (hasCorridorRegions) {
    ctx.fillStyle = '#9a8060'
    for (const edge of d.edges) for (const shape of edge.region) (pathOf(shape), ctx.fill())
  } else {
    ctx.strokeStyle = '#9a8060'
    ctx.lineWidth = P.width
    ctx.lineCap = 'butt'
    ctx.lineJoin = 'round'
    for (const edge of d.edges) {
      ctx.beginPath()
      edge.pts.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)))
      ctx.stroke()
    }
  }
  ctx.strokeStyle = 'rgba(34, 25, 18, .58)'
  ctx.lineWidth = px
  ctx.lineCap = 'butt'
  ctx.lineJoin = 'round'
  for (const edge of d.edges) {
    ctx.beginPath()
    edge.pts.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)))
    ctx.stroke()
  }
  for (const edge of d.edges) {
    if (!edge.requiresKeys && !edge.requiresSwitches) continue
    const lengths = edge.pts.slice(1).map((point, i) => Math.hypot(point[0] - edge.pts[i][0], point[1] - edge.pts[i][1]))
    let remaining = lengths.reduce((sum, length) => sum + length, 0) / 2
    let at = edge.pts[0]
    for (let i = 0; i < lengths.length; i++) {
      if (remaining <= lengths[i]) {
        const t = lengths[i] ? remaining / lengths[i] : 0
        at = [edge.pts[i][0] + (edge.pts[i + 1][0] - edge.pts[i][0]) * t, edge.pts[i][1] + (edge.pts[i + 1][1] - edge.pts[i][1]) * t]
        break
      }
      remaining -= lengths[i]
    }
    ctx.beginPath()
    ctx.arc(at[0], at[1], 8 * px, 0, Math.PI * 2)
    ctx.fillStyle = '#18212b'
    ctx.fill()
    ctx.strokeStyle = '#ffd36d'
    ctx.lineWidth = 2 * px
    ctx.stroke()
    ctx.font = `bold ${9 * px}px system-ui, sans-serif`
    ctx.textAlign = 'center'
    ctx.textBaseline = 'middle'
    ctx.fillStyle = '#ffe9ae'
    ctx.fillText(edge.requiresKeys ? 'K' : 'S', at[0], at[1] + 0.5 * px)
  }
  for (const room of d.rooms) {
    if (!room.alive) continue
    pathOf(room.shape)
    ctx.fillStyle = COLORS[room.cls] ?? '#59677a'
    ctx.globalAlpha = room.leaf ? 0.62 : 0.9
    ctx.fill()
    ctx.globalAlpha = 1
    ctx.strokeStyle = ROLE_COLORS[room.role] ?? '#18212d'
    ctx.lineWidth = (ROLE_COLORS[room.role] ? 3.5 : 1.5) * px
    ctx.stroke()
  }
  for (const room of d.rooms) {
    if (room.alive) continue
    pathOf(room.shape)
    ctx.strokeStyle = '#fb7474'
    ctx.setLineDash([4 * px, 4 * px])
    ctx.lineWidth = px
    ctx.stroke()
    ctx.setLineDash([])
  }
  if (overlays.ports) {
    ctx.fillStyle = 'rgba(255, 255, 255, .28)'
    for (const room of d.rooms) for (const port of room.ports) ctx.fillRect(port.p[0] - 1.5 * px, port.p[1] - 1.5 * px, 3 * px, 3 * px)
  }
  ctx.strokeStyle = '#fff'
  ctx.lineWidth = 2 * px
  for (const room of d.rooms) for (const port of room.used) {
    const direction = DIRS[port.k]
    ctx.beginPath()
    ctx.moveTo(port.p[0], port.p[1])
    ctx.lineTo(port.p[0] + direction[0] * 14, port.p[1] + direction[1] * 14)
    ctx.stroke()
  }
  for (const node of d.nodes) {
    if (node.kind !== 'junction') continue
    ctx.beginPath()
    ctx.arc(node.p[0], node.p[1], 5, 0, Math.PI * 2)
    ctx.fillStyle = '#fff'
    ctx.fill()
    ctx.strokeStyle = '#202a38'
    ctx.lineWidth = px
    ctx.stroke()
  }
  if (overlays.labels) {
    ctx.font = `${12 * px}px system-ui, sans-serif`
    ctx.textAlign = 'center'
    ctx.textBaseline = 'alphabetic'
    for (const room of d.rooms) {
      if (!room.alive) continue
      const inventory = [room.keys ? `K${room.keys.toString(2).replaceAll('0', '').length}` : '', room.switches ? `S${room.switches.toString(2).replaceAll('0', '').length}` : ''].filter(Boolean).join(' ')
      const label = `#${room.id} ${CLASS_LABELS[room.cls] ?? room.cls}${room.role !== 'plain' ? ` · ${ROLE_LABELS[room.role]}` : ''}${inventory ? ` · ${inventory}` : ''}`
      ctx.lineWidth = 3 * px
      ctx.strokeStyle = '#080d14'
      ctx.strokeText(label, room.center[0], room.center[1] + 4 * px)
      ctx.fillStyle = '#f1f5fb'
      ctx.fillText(label, room.center[0], room.center[1] + 4 * px)
    }
  }
  zoomLabel.textContent = `${Math.round(view.zoom * 100)}%`
}

/* ---------------- Detalhes ao explorar o mapa ---------------- */

const containsPoint = (shape, point) => {
  if (shape.kind === 'disc') return Math.hypot(point[0] - shape.c[0], point[1] - shape.c[1]) <= shape.r
  let inside = false
  const pts = shape.pts
  for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
    const [xi, yi] = pts[i]
    const [xj, yj] = pts[j]
    const crosses = (yi > point[1]) !== (yj > point[1]) && point[0] < ((xj - xi) * (point[1] - yi)) / (yj - yi) + xi
    if (crosses) inside = !inside
  }
  return inside
}

const distanceToSegment = (point, a, b) => {
  const dx = b[0] - a[0]
  const dy = b[1] - a[1]
  const length2 = dx * dx + dy * dy
  const t = length2 ? Math.max(0, Math.min(1, ((point[0] - a[0]) * dx + (point[1] - a[1]) * dy) / length2)) : 0
  return Math.hypot(point[0] - (a[0] + t * dx), point[1] - (a[1] + t * dy))
}

const roomRoleText = (room) => room.leaf ? `câmara-folha · ${ROLE_LABELS[room.role] ?? 'vazia'}` : (ROLE_LABELS[room.role] ?? 'sala comum')
const nodeLabel = (dungeon, nodeId) => {
  const node = dungeon.nodes[nodeId]
  if (node?.kind === 'junction') return 'junção'
  const room = dungeon.rooms[node?.room]
  return room ? `sala #${room.id}` : 'ponto'
}

const mapObjectAt = (point, scale) => {
  const d = current.dungeon
  for (const room of [...d.rooms].reverse()) {
    if (room.alive && containsPoint(room.shape, point)) {
      const inventory = [room.keys ? `chaves: ${room.keys.toString(2).replaceAll('0', '').length}` : '', room.switches ? `interruptores: ${room.switches.toString(2).replaceAll('0', '').length}` : ''].filter(Boolean)
      return `${roomRoleText(room)}\nSala #${room.id} · ${CLASS_LABELS[room.cls] ?? room.cls}\nÁrea: ${formatNumber(room.area, 0)} px²\nPortas usadas: ${room.used.length} · portas possíveis: ${room.ports.length}${inventory.length ? `\nItens: ${inventory.join(' · ')}` : ''}`
    }
  }
  const junction = d.nodes.find((node) => node.kind === 'junction' && Math.hypot(node.p[0] - point[0], node.p[1] - point[1]) <= Math.max(6, 8 / scale))
  if (junction) {
    const id = d.nodes.indexOf(junction)
    const degree = d.edges.filter((edge) => edge.u === id || edge.v === id).length
    return `Junção de corredor\n${degree} braços conectados\nPosição: ${formatNumber(junction.p[0], 0)}, ${formatNumber(junction.p[1], 0)} px`
  }
  const threshold = current.dungeon.params.width / 2 + 5 / scale
  for (const edge of d.edges) {
    for (let i = 1; i < edge.pts.length; i++) {
      if (distanceToSegment(point, edge.pts[i - 1], edge.pts[i]) <= threshold) {
        const length = edge.pts.slice(1).reduce((sum, p, j) => sum + Math.hypot(p[0] - edge.pts[j][0], p[1] - edge.pts[j][1]), 0)
        const gates = [
          edge.requiresKeys ? `chave${edge.requiresKeys.toString(2).replaceAll('0', '').length > 1 ? 's' : ''} ${edge.requiresKeys.toString(2)}` : '',
          edge.requiresSwitches ? `interruptor${edge.requiresSwitches.toString(2).replaceAll('0', '').length > 1 ? 'es' : ''} ${edge.requiresSwitches.toString(2)}` : '',
        ].filter(Boolean)
        return `Trecho de corredor #${edge.id}\n${nodeLabel(d, edge.u)} → ${nodeLabel(d, edge.v)}\n${edge.pts.length - 1} segmento(s) · ${formatNumber(length, 0)} px${gates.length ? `\nTrava: exige ${gates.join(' e ')}` : ''}`
      }
    }
  }
  return ''
}

const showMapTooltip = (event) => {
  if (!current) return
  const rect = canvas.getBoundingClientRect()
  const { k, ox, oy } = transformOf()
  const point = [(event.clientX - rect.left - ox) / k, (event.clientY - rect.top - oy) / k]
  const content = mapObjectAt(point, k)
  if (!content) {
    mapTooltip.hidden = true
    return
  }
  mapTooltip.textContent = content
  mapTooltip.hidden = false
  const viewRect = viewSection.getBoundingClientRect()
  mapTooltip.style.left = `${Math.max(10, Math.min(event.clientX - viewRect.left + 14, viewRect.width - 250))}px`
  mapTooltip.style.top = `${Math.max(58, Math.min(event.clientY - viewRect.top + 14, viewRect.height - 100))}px`
}

const hideMapTooltip = () => { mapTooltip.hidden = true }

/* ---------------- Painel de métricas ---------------- */

const right = h('section', { id: 'results', class: 'panel', 'aria-label': 'Validação e medidas do andar' })
const METRIC_INFO = {
  rooms: 'Salas principais do andar. As câmaras-folha aparecem separadas.',
  leafChambers: 'Salas extras penduradas por um único corredor, como um beco.',
  junctions: 'Pontos onde três ou quatro braços de corredor se encontram.',
  edges: 'Trechos do grafo entre salas e junções. Cada braço de uma junção conta como um trecho.',
  beta: 'Número de ciclos independentes. Uma árvore tem zero; cada novo atalho costuma acrescentar um.',
  betaTarget: 'Alvo de ciclos calculado a partir da densidade escolhida e da referência do gerador.',
  betaMax: 'Referência gulosa de quantos ciclos cabem. Considera ligações diretas e junções permitidas.',
  delta: 'Proporção entre ciclos atuais e a referência βmax. Um valor maior costuma significar mais rotas alternativas.',
  meanDegree: 'Média de portas de corredor usadas por sala.',
  leaves: 'Salas com apenas uma porta. Geralmente são becos, entrada ou escada.',
  bridges: 'Trechos cuja remoção separaria uma parte do mapa. Muitos indicam gargalos.',
  articulations: 'Salas cuja remoção dividiria o mapa em partes desconectadas.',
  hopsToStairs: 'Quantidade de salas atravessadas no caminho mais curto da entrada à escada.',
  diameter: 'Maior distância mínima, em passos, entre duas salas do mapa.',
  routesToStairs: 'Quantidade de caminhos entre entrada e escada que não compartilham trechos de corredor.',
  cycleGirth: 'Comprimento, em trechos, do menor ciclo do mapa. “—” significa que o grafo não tem ciclos.',
  sideBranches: 'Número de componentes laterais presos ao caminho principal por uma única passagem.',
  deepestBranch: 'Maior número de salas em um ramo lateral, contando a sala conectada ao caminho principal.',
  maxNodeDegree: 'Maior número de braços de corredor ligados a um mesmo ponto ou sala.',
  geometricPathToStairs: 'Comprimento total em pixels do caminho com menos trechos entre entrada e escada.',
  lambda2: 'Medida de conectividade: valores maiores tendem a indicar um mapa menos dependente de gargalos.',
}

const metricRow = (label, value, description) => h('tr', {}, h('th', { scope: 'row' }, label, help(description)), h('td', {}, value))

const translateViolation = (message) => {
  let match
  if ((match = message.match(/^V1 room (\d+) not reached from the start$/))) return `A sala #${match[1]} não é alcançável a partir da entrada.`
  if ((match = message.match(/^V2 rooms (\d+) and (\d+) closer than 2g$/))) return `As salas #${match[1]} e #${match[2]} estão próximas demais.`
  if ((match = message.match(/^V4 corridor (.+) reaches (\d+) room\(s\): every corridor joins rooms$/))) return `O corredor ${match[1]} alcança apenas ${match[2]} sala(s); todo corredor precisa ligar salas.`
  if ((match = message.match(/^V4 rooms (\d+) and (\d+): corridor (.+) px < ℓmin$/))) return `O caminho entre as salas #${match[1]} e #${match[2]} é curto demais (${match[3]} px).`
  if ((match = message.match(/^V2 rooms (\d+) and (\d+) joined but closer than ℓmin in a straight line$/))) return `As salas #${match[1]} e #${match[2]} estão próximas demais em linha reta.`
  if ((match = message.match(/^V2 corridor (\d+) runs into room (\d+) past its door$/))) return `O corredor #${match[1]} invade a sala #${match[2]} além da porta.`
  if ((match = message.match(/^V2 corridor (\d+) closer than σ to room (\d+)$/))) return `O corredor #${match[1]} está perto demais da sala #${match[2]}.`
  if ((match = message.match(/^V2 corridors (\d+) and (\d+) closer than σ$/))) return `Os corredores #${match[1]} e #${match[2]} estão perto demais.`
  if ((match = message.match(/^V3 room (\d+): a door at 45° in a retilinear stage$/))) return `A sala #${match[1]} tem uma porta diagonal num andar retangular.`
  if ((match = message.match(/^V3 room (\d+): a door's exit band runs back into the room$/))) return `A saída da porta da sala #${match[1]} volta para dentro dela.`
  if ((match = message.match(/^V3 room (\d+): a door off the circle$/))) return `Uma porta da sala circular #${match[1]} não está na borda.`
  if ((match = message.match(/^V3 room (\d+): doors (\d+) and (\d+) closer than σ$/))) return `Duas portas da sala #${match[1]} estão próximas demais.`
  if ((match = message.match(/^V4 corridor (\d+): a segment off the alphabet$/))) return `O corredor #${match[1]} usa uma direção não permitida.`
  if ((match = message.match(/^V4 corridor (\d+): a (\d+)° turn$/))) return `O corredor #${match[1]} tem uma dobra de ${match[2]}°, que não é permitida.`
  if ((match = message.match(/^V4 corridor (\d+): a segment of (.+) px$/))) return `Um segmento do corredor #${match[1]} é curto demais (${match[2]} px).`
  if ((match = message.match(/^V4 corridor (\d+): leaves a door at an angle$/))) return `O corredor #${match[1]} sai de uma porta na direção errada.`
  if ((match = message.match(/^V5 the stairs have (\d+) doors \(most (\d+)\)$/))) return `A escada tem ${match[1]} portas; o máximo configurado é ${match[2]}.`
  if ((match = message.match(/^V5 the stairs are (.+) hops from the start \(least (\d+)\)$/))) return `A escada está a ${match[1]} passos da entrada; o mínimo é ${match[2]}.`
  if (message === 'V5 start and stairs must be two rooms of the stage') return 'A entrada e a escada precisam ser duas salas distintas do andar.'
  if ((match = message.match(/^T1 profile requires (\d+) side branches; found (\d+)$/))) return `O perfil exige ${match[1]} ramos laterais; foram gerados ${match[2]}.`
  if ((match = message.match(/^T2 profile requires branch depth (\d+); found (\d+)$/))) return `O perfil exige profundidade de ramo ${match[1]}; foi gerada ${match[2]}.`
  if ((match = message.match(/^T3 profile requires (\d+) independent routes to stairs; found (\d+)$/))) return `O perfil exige ${match[1]} rotas independentes até a escada; foram geradas ${match[2]}.`
  if ((match = message.match(/^T4 profile requires a hub degree of (\d+); found (\d+)$/))) return `O perfil exige um ponto com pelo menos ${match[1]} braços; o maior tem ${match[2]}.`
  if ((match = message.match(/^T5 profile requires a cycle of at least (\d+) branches; found (\d+)$/))) return `O perfil exige um ciclo de pelo menos ${match[1]} trechos; o menor tem ${match[2]}.`
  if (message === 'P1 stairs are unreachable with the generated keys and switches') return 'A escada não pode ser alcançada com as chaves e os interruptores disponíveis.'
  if (message === 'P2 a required key is unreachable before its gate') return 'Uma chave necessária não pode ser recolhida antes do portão correspondente.'
  if (message === 'P3 a required switch is unreachable before its gate') return 'Um interruptor necessário não pode ser ativado antes do portão correspondente.'
  return message
}

const translateLog = (message) => {
  let match
  if ((match = message.match(/^tree stuck: candidates from (.+)$/i))) return `Poucas conexões disponíveis; ampliando a busca (${match[1]}).`
  if ((match = message.match(/^tree stuck: near (.+) px$/i))) return `Ampliando o raio de busca para ${match[1]} px.`
  if ((match = message.match(/^(.+) room\(s\) left out: no valid corridor reaches them$/))) return `${match[1]} sala(s) ficaram isoladas porque nenhuma ligação válida foi encontrada.`
  if ((match = message.match(/^stairs only (.+) hops from the start \(wanted (.+)\)$/))) return `A escada ficou a ${match[1]} passos da entrada; o alvo era ${match[2]}.`
  if ((match = message.match(/^β\* = (.+) not reached: nothing more fits \(β = (.+)\)$/))) return `O alvo de ${match[1]} ciclos não foi alcançado; o mapa ficou com ${match[2]}.`
  if ((match = message.match(/^block (\d+): (.+) does not fit, rect instead$/))) return `A forma ${match[2]} não coube no bloco ${match[1]}; foi usada uma sala retangular.`
  if ((match = message.match(/^(.+) of (.+) leaf chambers fit$/))) return `Couberam ${match[1]} de ${match[2]} câmaras-folha.`
  if ((match = message.match(/^progression: only (\d+) of (\d+) requested locks fit on the main path$/))) return `Só couberam ${match[1]} das ${match[2]} travas pedidas no caminho principal.`
  if (message === 'progression: each lock type is limited to eight bits') return 'Cada tipo de trava aceita no máximo oito itens.'
  if ((match = message.match(/^topology profile: could not reach (\d+) independent routes to stairs \(λ = (\d+)\)$/))) return `O perfil chegou a ${match[2]} de ${match[1]} rotas independentes até a escada.`
  if ((match = message.match(/^topology profile: no valid junction can reach degree (\d+)$/))) return `Não coube uma junção com grau ${match[1]}.`
  if ((match = message.match(/^topology profile: no valid cycle of at least (\d+) branches fits$/))) return `Não coube um ciclo com pelo menos ${match[1]} trechos.`
  if ((match = message.match(/^topology profile: added (\d+) of (\d+) required side branches$/))) return `Foram acrescentados ${match[1]} de ${match[2]} ramos laterais exigidos.`
  if ((match = message.match(/^layout jitter limited to (.+) \(requested (.+)\) to preserve minimum room size$/))) return `A irregularidade da grade foi limitada a ${match[1]} (pedido: ${match[2]}) para preservar o tamanho mínimo das salas.`
  if (/shape class caps cannot fit/i.test(message)) return 'Os limites de formas não comportam todas as salas; algum limite foi excedido.'
  return message
}

const renderPanel = () => {
  right.replaceChildren()
  if (!current) return
  const result = current
  const m = result.metrics
  const status = result.violations.length === 0
    ? h('div', { class: 'validity-card valid' }, h('span', { class: 'status-icon', 'aria-hidden': 'true' }, '✓'), h('div', {}, h('strong', {}, 'Mapa válido'), h('p', {}, `${result.draws} tentativa(s) · ${formatNumber(result.ms, 1)} ms de geração`)))
    : h('div', { class: 'validity-card invalid' }, h('span', { class: 'status-icon', 'aria-hidden': 'true' }, '!'), h('div', {}, h('strong', {}, `${result.violations.length} problema(s) encontrado(s)`), h('p', {}, `Após ${result.draws} tentativa(s), o mapa ainda não atende a todas as regras.`), h('ul', {}, ...result.violations.slice(0, 30).map((item) => h('li', {}, translateViolation(item))))))
  right.append(h('div', { class: 'panel-header' }, h('p', { class: 'eyebrow' }, `NÍVEL ${state.level} · SEMENTE ${state.seed}`), h('h2', {}, 'Leitura do andar')), status)

  const generalRows = [
    ['Salas principais', `${m.rooms}`, METRIC_INFO.rooms],
    ['Câmaras-folha', `${m.leafChambers}`, METRIC_INFO.leafChambers],
    ['Junções T/X', `${m.junctions}`, METRIC_INFO.junctions],
    ['Trechos de corredor', `${m.edges}`, METRIC_INFO.edges],
  ]
  const cycleRows = [
    ['Ciclos atuais β', `${m.beta}`, METRIC_INFO.beta],
    ['Alvo de ciclos', `${m.betaTarget}`, METRIC_INFO.betaTarget],
    ['Referência βmax', `${m.betaMax}`, METRIC_INFO.betaMax],
    ['Densidade δ', `${formatNumber(m.delta, 2)}`, METRIC_INFO.delta],
  ]
  const routeRows = [
    ['Portas usadas por sala', formatNumber(m.meanDegree, 2), METRIC_INFO.meanDegree],
    ['Salas de uma porta', `${m.leaves}`, METRIC_INFO.leaves],
    ['Pontes (fração)', `${m.bridges} · ${formatNumber(m.bridgeShare * 100, 0)}%`, METRIC_INFO.bridges],
    ['Salas de articulação', `${m.articulations}`, METRIC_INFO.articulations],
    ['Passos até a escada', m.hopsToStairs == null ? '—' : formatNumber(m.hopsToStairs, 0), METRIC_INFO.hopsToStairs],
    ['Diâmetro do mapa', formatNumber(m.diameter, 0), METRIC_INFO.diameter],
    ['Rotas independentes até a escada', `${m.routesToStairs}`, METRIC_INFO.routesToStairs],
    ['Menor ciclo (trechos)', m.cycleGirth == null ? '—' : `${m.cycleGirth}`, METRIC_INFO.cycleGirth],
    ['Ramos laterais', `${m.sideBranches}`, METRIC_INFO.sideBranches],
    ['Profundidade dos ramos', `${m.deepestBranch}`, METRIC_INFO.deepestBranch],
    ['Maior grau de um ponto', `${m.maxNodeDegree}`, METRIC_INFO.maxNodeDegree],
    ['Caminho geométrico até a escada', `${formatNumber(m.geometricPathToStairs, 0)} px`, METRIC_INFO.geometricPathToStairs],
    ['Conectividade λ₂', formatNumber(m.lambda2, 3), METRIC_INFO.lambda2],
  ]
  const tableFor = (rows) => h('table', { class: 'metric-table' }, h('tbody', {}, ...rows.map(([label, value, description]) => metricRow(label, value, description))))
  right.append(
    h('details', { class: 'result-group', open: '' }, h('summary', {}, 'Tamanho do mapa'), tableFor(generalRows)),
    h('details', { class: 'result-group', open: '' }, h('summary', {}, 'Ciclos e densidade'), tableFor(cycleRows)),
    h('details', { class: 'result-group' }, h('summary', {}, 'Caminhos e conectividade'), tableFor(routeRows)),
  )

  const classCounts = Object.entries(m.byClass).map(([name, count]) => h('span', { class: 'shape-chip' }, h('i', { style: `background:${COLORS[name] ?? '#59677a'}`, 'aria-hidden': 'true' }), `${CLASS_LABELS[name] ?? name}: ${count}`))
  right.append(h('section', { class: 'result-section' }, h('h3', {}, 'Formas no mapa'), h('div', { class: 'chip-list' }, ...classCounts)))

  if (result.dungeon.log.length) {
    right.append(h('details', { class: 'result-group' }, h('summary', {}, 'O que o gerador fez'), h('ul', { class: 'log-list' }, ...result.dungeon.log.map((line) => h('li', {}, translateLog(line))))))
  }

  const shapeLegend = CLASSES.map((name) => h('span', { class: 'legend-item' }, h('i', { style: `background:${COLORS[name]}`, 'aria-hidden': 'true' }), CLASS_LABELS[name]))
  const roleLegend = Object.keys(ROLE_COLORS).map((name) => h('span', { class: 'legend-item' }, h('i', { style: `background:${ROLE_COLORS[name]}`, 'aria-hidden': 'true' }), ROLE_LABELS[name]))
  right.append(h('details', { class: 'result-group' }, h('summary', {}, 'Legenda do mapa'), h('div', { class: 'legend-block' }, h('strong', {}, 'Formas'), h('div', { class: 'legend-list' }, ...shapeLegend), h('strong', {}, 'Papéis das salas'), h('div', { class: 'legend-list' }, ...roleLegend), h('p', { class: 'muted' }, 'Traços brancos mostram portas usadas; pontos brancos são junções. Salas tracejadas em vermelho ficaram fora da rede.'))))
}

/* ---------------- Controles e cabeçalho ---------------- */

const levelButtons = [1, 2, 3, 4, 5].map((level) => {
  const button = h('button', { type: 'button', class: 'level-button', 'aria-label': `Usar configuração do nível ${level}` }, `Nível ${level}`)
  button.addEventListener('click', () => setLevel(level))
  return button
})
const seedInput = h('input', { type: 'number', min: '0', max: '4294967295', step: '1', value: '1', 'aria-label': 'Semente aleatória' })
const levelDescription = h('p', { class: 'level-description' }, LEVEL_INFO[0])
const changedStatus = h('span', { class: 'changed-status' }, 'Configuração padrão')
const toast = h('div', { class: 'toast', role: 'status', 'aria-live': 'polite', hidden: '' })

const showToast = (message) => {
  toast.textContent = message
  toast.hidden = false
  window.clearTimeout(showToast.timer)
  showToast.timer = window.setTimeout(() => { toast.hidden = true }, 2200)
}

const updateLevelButtons = () => {
  levelButtons.forEach((button, index) => {
    const selected = index + 1 === state.level
    button.classList.toggle('selected', selected)
    button.setAttribute('aria-pressed', String(selected))
  })
  levelDescription.textContent = LEVEL_INFO[state.level - 1]
}

const seedStep = (amount) => {
  state.seed = Math.max(0, Math.min(0xffffffff, state.seed + amount))
  seedInput.value = String(state.seed)
  scheduleRegenerate()
}

const newSeed = () => {
  state.seed = Math.floor(Math.random() * 1e6)
  seedInput.value = String(state.seed)
  scheduleRegenerate()
}

seedInput.addEventListener('change', () => {
  const seed = Number(seedInput.value)
  if (!Number.isInteger(seed) || seed < 0 || seed > 0xffffffff) {
    seedInput.setAttribute('aria-invalid', 'true')
    seedInput.value = String(state.seed)
    showToast('A semente deve ser um inteiro entre 0 e 4.294.967.295.')
    return
  }
  seedInput.removeAttribute('aria-invalid')
  state.seed = seed
  scheduleRegenerate()
})

const performanceStatus = h('div', { class: 'status-line', 'aria-live': 'polite' }, 'Os testes usam os parâmetros padrão do nível selecionado.')
const performanceGroup = h('details', { class: 'tool-group' }, h('summary', {}, 'Desempenho e exportação'), h('p', { class: 'group-intro' }, 'Os benchmarks usam a configuração padrão do nível, não os valores alterados no formulário.'), h('div', { class: 'tool-buttons' }))
const toolButtons = performanceGroup.querySelector('.tool-buttons')

toolButtons.append(
  h('button', { type: 'button', class: 'secondary-button' }, 'Medir 40 mapas'),
  h('button', { type: 'button', class: 'secondary-button' }, 'Medir lote de 800'),
)
toolButtons.children[0].addEventListener('click', () => {
  const result = JSON.parse(bench(state.level, 1, 40))
  performanceStatus.textContent = `Média: ${formatNumber(result.ms / 40, 2)} ms por mapa · ${result.invalid} inválido(s) de 40.`
})
toolButtons.children[1].addEventListener('click', async () => {
  performanceStatus.textContent = 'Medindo o lote…'
  try {
    const one = await pool.batch(state.level, 1, 800, 1)
    const all = await pool.batch(state.level, 1, 800)
    performanceStatus.textContent = `800 mapas: 1 trabalhador ${formatNumber(one.ms, 0)} ms · ${all.workers} trabalhadores ${formatNumber(all.ms, 0)} ms · ${all.invalid} inválido(s).`
  } catch (error) {
    performanceStatus.textContent = `Não foi possível medir o lote: ${error}`
  }
})
performanceGroup.append(performanceStatus)

const overlaysHelp = {
  cells: 'Mostra os limites das células que formam a grade do andar.',
  labels: 'Mostra o número, a forma e o papel de cada sala.',
  ports: 'Mostra todas as posições de porta possíveis, inclusive as que não foram usadas.',
  pairs: 'Mostra os pares de salas que entraram na busca de conexões.',
  candidates: 'Mostra os caminhos candidatos disponíveis para o gerador.',
}
const overlayNames = { cells: 'Grade', labels: 'Nomes das salas', ports: 'Portas possíveis', pairs: 'Pares considerados', candidates: 'Caminhos candidatos' }
const overlayList = h('div', { class: 'overlay-list' })
for (const key of Object.keys(overlays)) {
  const input = h('input', { type: 'checkbox', checked: overlays[key] ? 'checked' : '', 'aria-label': overlayNames[key] })
  input.checked = overlays[key]
  input.addEventListener('change', () => { overlays[key] = input.checked; draw() })
  overlayList.append(h('label', { class: 'overlay-option' }, input, h('span', {}, overlayNames[key]), help(overlaysHelp[key])))
}

const experiments = h('details', { class: 'tool-group experiments' }, h('summary', {}, 'Experimentos guiados'), h('p', { class: 'group-intro' }, 'Aplique uma mudança de exemplo para observar o efeito. Isso substitui apenas os parâmetros indicados; você pode editar o restante normalmente.'), h('div', { class: 'experiment-buttons' }))
const experimentButtons = experiments.querySelector('.experiment-buttons')
const runExperiment = (kind) => {
  if (kind === 'shapes') {
    for (const shape of CLASSES) state.P.weights[shape] = 1
    state.P.m = 8
    showToast('Experimento aplicado: formas variadas e direções diagonais.')
  } else if (kind === 'cycles') {
    state.P.delta = 0.8
    showToast('Experimento aplicado: densidade de ciclos elevada.')
  } else if (kind === 'junctions') {
    state.P.p_t = Math.max(state.P.p_t, 0.25)
    state.P.p_x = Math.max(state.P.p_x, 0.15)
    showToast('Experimento aplicado: junções T e X favorecidas.')
  } else if (kind === 'rooms') {
    for (const shape of CLASSES) state.P.weights[shape] = shape === 'disc' ? 1 : 0
    showToast('Experimento aplicado: preferência por salas circulares.')
  }
  buildForm()
  updateChangedIndicator()
  scheduleRegenerate()
}
for (const [kind, label, detail] of [
  ['shapes', 'Misturar formas', 'Ativa pesos iguais para as formas e o alfabeto octilinear.'],
  ['cycles', 'Criar mais rotas', 'Aumenta a densidade-alvo para favorecer ciclos e atalhos.'],
  ['junctions', 'Ver junções T/X', 'Aumenta os pesos de junções de três e quatro braços.'],
  ['rooms', 'Usar salas circulares', 'Faz salas circulares dominarem o sorteio.'],
]) {
  const button = h('button', { type: 'button', class: 'experiment-button' }, h('strong', {}, label), h('span', {}, detail))
  button.addEventListener('click', () => runExperiment(kind))
  experimentButtons.append(button)
}

const copyButton = h('button', { type: 'button', class: 'secondary-button' }, 'Copiar parâmetros (JSON)')
copyButton.addEventListener('click', async () => {
  try {
    await navigator.clipboard.writeText(JSON.stringify({ level: state.level, seed: state.seed, params: state.P }, null, 2))
    showToast('Parâmetros copiados em JSON.')
  } catch {
    showToast('A área de transferência não está disponível neste navegador.')
  }
})

const restoreButton = h('button', { type: 'button', class: 'secondary-button' }, 'Restaurar padrão do nível')
restoreButton.addEventListener('click', () => setLevel(state.level))

const left = h('section', { id: 'controls', class: 'panel', 'aria-label': 'Controles do gerador' })
left.append(
  h('div', { class: 'brand-row' }, h('div', { class: 'brand-mark', 'aria-hidden': 'true' }, '⌘'), h('div', {}, h('p', { class: 'eyebrow' }, 'LABORATÓRIO INTERATIVO'), h('h1', {}, 'Gerador de dungeons'))),
  h('p', { class: 'intro-copy' }, 'Explore como as regras matemáticas mudam a forma e os caminhos de um andar. Altere um valor para gerar um novo mapa.'),
  h('div', { class: 'level-section' }, h('div', { class: 'section-title' }, h('h2', {}, 'Escolha um nível'), help('Cada nível é um conjunto inicial de parâmetros. Você pode editar qualquer valor depois.')), h('div', { class: 'level-list' }, ...levelButtons), levelDescription),
  h('div', { class: 'seed-card' }, h('label', { for: 'seed-input' }, 'Semente do mapa', help('A mesma semente e os mesmos parâmetros geram o mesmo mapa. Mude a semente para explorar outra configuração aleatória.')), h('div', { class: 'seed-control' }, seedInput, h('button', { type: 'button', class: 'icon-button', 'aria-label': 'Semente anterior', title: 'Semente anterior' }, '←'), h('button', { type: 'button', class: 'icon-button', 'aria-label': 'Próxima semente', title: 'Próxima semente' }, '→'), h('button', { type: 'button', class: 'secondary-button' }, 'Nova semente')), h('p', { class: 'keyboard-hint' }, 'Atalhos: 1–5 níveis · ← → sementes · R nova semente')),
  h('div', { class: 'action-row' }, copyButton, restoreButton),
  h('div', { class: 'changed-row' }, changedStatus),
  h('details', { class: 'howto' }, h('summary', {}, 'Como explorar este mapa'), h('p', {}, 'Arraste o mapa para mover. Use a roda do mouse ou os botões de zoom. Passe o cursor sobre uma sala, junção ou corredor para ver detalhes.')),
  h('div', { class: 'section-title overlay-title' }, h('h2', {}, 'O que mostrar'), help('As camadas ajudam a entender como o gerador montou o mapa. Ative uma de cada vez para comparar sem esconder as salas.')),
  overlayList,
  experiments,
  h('div', { class: 'section-title parameter-title' }, h('h2', {}, 'Parâmetros'), help('As mudanças são aplicadas ao mapa automaticamente. Cada campo tem uma explicação e limites recomendados.')),
  form,
  performanceGroup,
)
left.querySelector('.seed-control .icon-button:nth-of-type(1)').addEventListener('click', () => seedStep(-1))
left.querySelector('.seed-control .icon-button:nth-of-type(2)').addEventListener('click', () => seedStep(1))
left.querySelector('.seed-control .secondary-button').addEventListener('click', newSeed)

/* ---------------- Ferramentas sobre o mapa ---------------- */

const zoomBy = (factor) => {
  const next = Math.max(0.3, Math.min(20, view.zoom * factor))
  const ratio = next / view.zoom
  view.x *= ratio
  view.y *= ratio
  view.zoom = next
  draw()
}
const resetView = () => { view.zoom = 1; view.x = 0; view.y = 0; draw() }

const toolbar = h('div', { class: 'map-toolbar', role: 'toolbar', 'aria-label': 'Controles do mapa' })
const zoomOutButton = h('button', { type: 'button', class: 'map-tool-button', 'aria-label': 'Diminuir zoom', title: 'Diminuir zoom' }, '−')
const zoomInButton = h('button', { type: 'button', class: 'map-tool-button', 'aria-label': 'Aumentar zoom', title: 'Aumentar zoom' }, '+')
const fitButton = h('button', { type: 'button', class: 'map-tool-button fit-button' }, 'Ajustar mapa')
zoomOutButton.addEventListener('click', () => zoomBy(0.8))
zoomInButton.addEventListener('click', () => zoomBy(1.25))
fitButton.addEventListener('click', resetView)
toolbar.append(zoomOutButton, zoomLabel, zoomInButton, fitButton)
viewSection.append(toolbar, h('div', { class: 'map-instructions', id: 'map-instructions' }, 'Arraste para mover · roda para zoom · passe sobre elementos para detalhes'))
canvas.setAttribute('aria-describedby', 'map-instructions')

/* ---------------- Layout e geração ---------------- */

const scheduleRegenerate = () => {
  clearTimeout(regenTimer)
  updateLevelButtons()
  window.clearTimeout(scheduleRegenerate.indicatorTimer)
  changedStatus.dataset.pending = 'true'
  window.setTimeout(() => { delete changedStatus.dataset.pending }, 180)
  regenTimer = window.setTimeout(regenerate, 80)
}

const regenerate = () => {
  try {
    current = JSON.parse(stage_json(state.seed >>> 0, JSON.stringify(state.P)))
    hideHelp()
    draw()
    renderPanel()
  } catch (error) {
    current = null
    right.replaceChildren(h('div', { class: 'validity-card invalid' }, h('strong', {}, 'Não foi possível gerar o mapa'), h('p', {}, String(error))))
    draw()
  }
}

function setLevel(level) {
  state.level = level
  state.P = JSON.parse(default_params(level))
  state.defaults = clone(state.P)
  updateLevelButtons()
  buildForm()
  updateChangedIndicator()
  scheduleRegenerate()
}

let drag = null
canvas.addEventListener('wheel', (event) => {
  event.preventDefault()
  const rect = canvas.getBoundingClientRect()
  const sx = event.clientX - rect.left - rect.width / 2
  const sy = event.clientY - rect.top - rect.height / 2
  const dy = Math.max(-100, Math.min(100, event.deltaY * (event.deltaMode === 1 ? 33 : 1)))
  const next = Math.max(0.3, Math.min(20, view.zoom * Math.exp(-dy * 0.0012)))
  const ratio = next / view.zoom
  view.x = sx - (sx - view.x) * ratio
  view.y = sy - (sy - view.y) * ratio
  view.zoom = next
  draw()
}, { passive: false })

canvas.addEventListener('pointerdown', (event) => {
  hideMapTooltip()
  drag = { x: event.clientX, y: event.clientY, moved: false }
  canvas.classList.add('dragging')
  canvas.setPointerCapture(event.pointerId)
})
canvas.addEventListener('pointermove', (event) => {
  if (!drag) {
    showMapTooltip(event)
    return
  }
  const dx = event.clientX - drag.x
  const dy = event.clientY - drag.y
  if (Math.abs(dx) + Math.abs(dy) > 2) drag.moved = true
  if (drag.moved) {
    view.x += dx
    view.y += dy
    drag.x = event.clientX
    drag.y = event.clientY
    hideMapTooltip()
    draw()
  }
})
canvas.addEventListener('pointerup', (event) => {
  const wasDrag = drag?.moved
  drag = null
  canvas.classList.remove('dragging')
  if (!wasDrag) showMapTooltip(event)
})
canvas.addEventListener('pointercancel', () => { drag = null; canvas.classList.remove('dragging'); hideMapTooltip() })
canvas.addEventListener('pointerleave', (event) => { if (!drag && event.pointerType !== 'touch') hideMapTooltip() })
canvas.addEventListener('dblclick', resetView)

window.addEventListener('resize', draw)
window.addEventListener('keydown', (event) => {
  if (event.target instanceof HTMLInputElement || event.target instanceof HTMLSelectElement || event.target?.isContentEditable) return
  if (event.key >= '1' && event.key <= '5') setLevel(Number(event.key))
  else if (event.key === 'r' || event.key === 'R') newSeed()
  else if (event.key === 'ArrowLeft') seedStep(-1)
  else if (event.key === 'ArrowRight') seedStep(1)
})

document.getElementById('app').replaceChildren(left, viewSection, right, toast)
updateLevelButtons()
buildForm()
updateChangedIndicator()
regenerate()
