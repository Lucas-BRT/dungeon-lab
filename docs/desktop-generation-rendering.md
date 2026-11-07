# Geração e preview ao vivo no desktop

## Objetivo

Cada solicitação interativa gera um andar e mostra sua construção no desktop. A geração continua fora da thread da interface, que recebe eventos pequenos de geometria para atualizar o mapa sem esperar o resultado final.

## Fluxo implementado

`stage_with_progress` encaminha eventos de cada tentativa: início da tentativa, grade pronta, salas criadas e trechos de corredor adicionados ou alterados. O gerador envia mudanças de topologia escolhidas pelo algoritmo. Simulações usadas apenas para calcular β máximo não aparecem no preview.

O desktop consome esses eventos numa stream do Iced. Um `work_id` descarta mensagens atrasadas de uma solicitação anterior. Quando a validação pede uma nova tentativa, o preview é limpo e passa a mostrar a construção dessa tentativa. Ao final, o mapa validado substitui o estado parcial e habilita inspeção, sobreposições, pan e zoom.

O preview é desenhado por um widget `Canvas` 2D. Durante a geração, ele mostra grade, salas e conexões; após a validação, desenha também portas, junções, rótulos e as sobreposições selecionadas. Pan, zoom e tema atualizam o desenho diretamente na interface, sem serializar SVG.

## Responsabilidades

- **Gerador:** executa numa thread de trabalho e publica alterações de geometria ao construir cada tentativa.
- **Interface:** aplica eventos da tentativa atual e redesenha o Canvas; a thread principal permanece disponível para interação.
- **Canvas:** desenha o estado atual do mapa; o cálculo do mapa continua sendo executado no CPU pelo gerador.
- **Validação:** determina qual tentativa se torna o mapa final. Desenhos de tentativas descartadas são substituídos quando a próxima tentativa começa.

## Medições

O painel registra o tempo total de `stage`, incluindo tentativas e validação. O custo de desenho ocorre durante a construção e não é somado ao tempo do gerador. Para comparar desempenho do algoritmo, use a duração de `stage`; para avaliar a experiência, observe se a interface mostra atualizações parciais e continua respondendo durante gerações maiores.

## Critérios

- Uma solicitação interativa gera um único andar, com tentativas internas de validação quando necessárias.
- A interface recebe eventos de sala e corredor antes do resultado final.
- O Canvas mostra as conexões à medida que o grafo do mapa é construído.
- Uma tentativa nova limpa eventos da tentativa anterior.
- Mensagens de solicitações antigas não substituem o mapa atual.
- O resultado validado substitui o preview parcial e oferece todas as inspeções e sobreposições existentes.
- O benchmark de vários mapas permanece separado do caminho interativo de um andar.
