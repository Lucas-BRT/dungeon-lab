# Geração e prévia ao vivo no desktop

## Objetivo

Cada solicitação interativa gera um andar e mostra como ele é construído no desktop. A geração continua fora da thread da interface, que recebe pequenos eventos de geometria para atualizar o mapa sem esperar o resultado final.

## Fluxo implementado

`stage_with_progress` encaminha eventos de cada tentativa: início da tentativa, grade pronta, salas criadas e trechos de corredor adicionados ou alterados. O gerador publica as mudanças de topologia que o algoritmo escolhe. Simulações usadas só para calcular o β máximo não aparecem na prévia.

O desktop recebe esses eventos por um canal mpsc; a thread de trabalho chama `egui::Context::request_repaint` depois de cada lote. Um `work_id` descarta mensagens atrasadas de uma solicitação anterior. Quando a validação pede uma nova tentativa, a prévia é limpa e passa a mostrar a construção dessa tentativa. No fim, o mapa validado substitui o estado parcial e habilita inspeção, camadas, pan e zoom.

A prévia é desenhada com o `Painter` do egui. Uma geração rápida (menos de 120 ms) troca o mapa de uma vez, sem piscar; uma lenta mostra o andar sendo montado. Durante a geração ela mostra a grade, as salas e as conexões; depois da validação desenha também portas, junções, rótulos e as camadas escolhidas. Pan, zoom e tema atualizam o desenho direto na interface, sem serializar SVG.

## Responsabilidades

- **Gerador:** roda numa thread de trabalho e publica mudanças de geometria ao construir cada tentativa.
- **Interface:** aplica os eventos da tentativa atual e redesenha o mapa; a thread principal continua livre para a interação.
- **Mapa:** desenha o estado atual do mapa; o cálculo do mapa continua rodando na CPU, no gerador.
- **Validação:** decide qual tentativa vira o mapa final. Os desenhos de tentativas descartadas são substituídos quando a próxima tentativa começa.

## Medições

O painel registra o tempo total de `stage`, incluindo tentativas e validação. O custo de desenho acontece durante a construção e não é somado ao tempo do gerador. Para comparar o desempenho do algoritmo, use a duração de `stage`; para avaliar a experiência, veja se a interface mostra atualizações parciais e continua respondendo em gerações maiores.

## Critérios

- Uma solicitação interativa gera um único andar, com tentativas internas de validação quando necessário.
- A interface recebe eventos de sala e corredor antes do resultado final.
- O mapa mostra as conexões à medida que o grafo é construído.
- Uma nova tentativa limpa os eventos da tentativa anterior.
- Mensagens de solicitações antigas não substituem o mapa atual.
- O resultado validado substitui a prévia parcial e oferece todas as inspeções e camadas existentes.
- O benchmark de vários mapas continua separado do caminho interativo de um andar.
