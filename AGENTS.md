# Instrucciones de desarrollo para agentes

## Proyecto
Simulador de redes educativo, de escritorio y open source, independiente de Cisco Packet Tracer. Primera tarea: validar un motor headless de eventos discretos mediante Spike E1 en Rust y C#.

## Fuente de verdad
- `docs/specs/SPEC-E1-v1.0.md`
- `docs/specs/SPEC-E1-addendum.md` (prevalece en caso de conflicto)

## Restricciones de E1
- No crear todavía Tauri, Vue, MCP, backend, plugins, UI, generador de proyectos ni CI compleja.
- No usar REA para inspeccionar software propietario sin autorización. No copiar código, nombres comerciales, imágenes o recursos de Cisco.
- No mezclar los dos prototipos mediante un motor compartido. Deben ser implementaciones independientes del mismo comportamiento especificado.
- Ambas implementaciones producen resultados observables comparables; su cantidad de eventos internos puede diferir.
- No usar tiempo real ni aleatoriedad sin semilla dentro de la simulación.
- Pruebas automatizadas antes de optimizaciones; no inventar resultados.
- No modificar la especificación sin documentar el motivo en un ADR pequeño.
- Ante una ambigüedad material, documentar la decisión mínima y su test; comunicarla en el resumen.

## Definición de terminado
- Los escenarios E1 implementados tienen pruebas ejecutables en Rust (`cargo test`) y C# (`dotnet test`).
- Se entregan instrucciones concretas para ejecutar ambos y un reporte de diferencias verificadas.
- Todas las pruebas ejecutadas y sus fallos constan explícitamente en el resumen. No declarar éxito si no se han ejecutado.
- Cualquier cambio de alcance debe justificarse.
