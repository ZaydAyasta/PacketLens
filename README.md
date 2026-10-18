# Network Simulator — Spike E1

Este es un **kit inicial de especificación**, no una implementación funcional. Diseñado para abrir en Codex y comenzar dos experimentos independientes de un motor de eventos discretos para redes: Rust y C#.

## Documentos

`docs/specs/SPEC-E1-v1.0.md`: especificación compartida original, aportada por el usuario.

`docs/specs/SPEC-E1-addendum.md`: correcciones acordadas que prevalecen sobre la especificación original para E1.

`AGENTS.md`: instrucciones de alcance y calidad para Codex y agentes compatibles.

`spikes/`: lugar reservado para los dos experimentos. Debe contener `spike_e1_rust` y `spike_e1_csharp` cuando se implementen.

## Inicio con Codex (Windows PowerShell)

Instalar por separado Rust stable-msvc, .NET 10 SDK, Git, Node.js compatible, Codex y las herramientas de compilación C++ para Windows. Desde el directorio raíz del proyecto:

```powershell
git init -b main
codex
```

Pedirle a Codex que inspeccione la especificación y el addendum, cree ambos prototipos headless y complete las pruebas sin crear aún la interfaz gráfica. Comenzar por un plan breve y ejecutar fases verificables. No asumir que REA hace parte del runtime del simulador.

## Estado

- [ ] Spikes creados
- [ ] Test Rust ejecutado
- [ ] Test C# ejecutado
- [ ] Reportes E1 comparados
- [ ] ADR de stack actualizado con resultados observados

El nombre comercial, la licencia open source y las decisiones sobre componentes finales siguen pendientes.
