# Arnés de evaluación E1

Este directorio contiene dos ejecutables pequeños e independientes para evaluar
los spikes E1. No forma parte de ningún motor y no contiene implementación de
protocolos. Ambos reproducen los comandos normativos E1-S1 a E1-S6 mediante la
API pública de su respectivo spike.

`reports` escribe un JSON canónico por línea. `workload <caso> <iteraciones>`
ejecuta una carga de medición sin escribir resultados intermedios. Los tiempos
del arnés, si se miden, son tiempo de pared del proceso de evaluación, no tiempo
virtual de la simulación.

La automatización reproducible y la metodología están documentadas en
`docs/reports/E1-ENGINE-COMPARISON-2026-10-18.md`.
