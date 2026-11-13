# Gera os programas do benchmark: análise de arquivos grandes e execução com muitas contas.
import sys
from pathlib import Path
d = Path(sys.argv[1]); d.mkdir(parents=True, exist_ok=True)
topo = 'data h = data"08:00:00";\nobjeto o = "Caixa";\nmensagem m = "Pegue a caixa.";\n'
etapa = '''    etapa("Etapa {i}") {{
        INDICAR(o);
        INSTRUIR(m);
        resultado r{i} = AGUARDAR_CONFIRMACAO(0s);
        PARAR_INDICACAO(o);
        q? (r{i} != CONFIRMADO && {i} % 2 == 0) {{
            REGISTRAR(ETAPA_PULADA);
        }}
    }}
'''
for n in (100, 1000, 10000):
    (d / f"analise_{n}.qjo").write_text(topo + 'rotina("R", h) {\n' + "".join(etapa.format(i=i) for i in range(n)) + "}\n")
(d / "execucao.qjo").write_text(topo + '''funcao num passo(num x) {
    retorna (x * 31 + 7) % 1000;
}
rotina("Contas", h) {
    num acc = 0;
    para (num i = 0; i < 150000; i = i + 1) {
        acc = passo(acc + i % 13);
        q? (acc == 999) {
            acc = 0;
        }
    }
    q? (acc > 0) {
        REGISTRAR(ETAPA_PULADA);
    }
}
''')
