# Medición de Interarmónicos — IEC 61000-4-30 §5.5 (Clase S)

## Método implementado

El cálculo de subgrupos interarmónicos se realiza mediante el **algoritmo de Goertzel
incremental** sobre una ventana de 200 ms (10 ciclos a 50 Hz). 

| Parámetro | Valor |
|-----------|-------|
| Ventana de medida | 10 ciclos (200 ms @ 50 Hz) |
| Frecuencia de muestreo sync | 25600 Hz (512 muestras/ciclo × 50 Hz) |
| Muestras totales por ventana | 5120 |
| Número de subgrupos | 49 (entre H1 y H50) |
| Frecuencias centrales | (i + 1.5) × 50 Hz = 75, 125, 175, ..., 2475 Hz |
| Algoritmo | Goertzel (filtro IIR de 2º orden) |
| Resultado | % de la magnitud fundamental |

## Algoritmo

Para cada subgrupo interarmónico `i` (0..48), con frecuencia central `fᵢ = (i + 1.5) × 50 Hz`:

```
coeff[i] = 2 · cos(2π · fᵢ / fₛ)

Para cada muestra x[n]:
    q0 = x[n] + coeff[i] · q1[i] − q2[i]
    q2[i] = q1[i]
    q1[i] = q0

Después de N = 5120 muestras:
    |X(fᵢ)|² = q1[i]² + q2[i]² − coeff[i] · q1[i] · q2[i]
    Amplitud = √(|X(fᵢ)|² / N²) · 2
    Subgrupo[i] = (Amplitud / Amplitud_fundamental) · 100 %
```

## Ventana de 10 ciclos (200 ms)

El estándar IEC 61000-4-30 §5.5 requiere una ventana de 10/12 ciclos (200 ms @ 50 Hz)
para proporcionar resolución espectral suficiente (5 Hz/bin). Con una ventana de 1 ciclo
(20 ms, resolución 50 Hz/bin), no hay bins entre armónicos y no es posible medir
interarmónicos.

Nuestro método:
1. Acumulamos 10 ciclos consecutivos de datos sincrónicamente remuestreados (512 muestras/ciclo)
2. Mantenemos el estado del filtro Goertzel (q1, q2) para cada una de las 49 frecuencias
3. Cada ciclo, procesamos las 512 muestras a través de los 49 filtros
4. Tras 10 ciclos, extraemos las magnitudes y reiniciamos los filtros

## Clase S: método a discreción del fabricante

IEC 61000-4-30:2021 §5.5 indica: *"For class S instrumentation, the method used for
interharmonic subgroup measurement is at the discretion of the manufacturer."*

Nuestro método declarado es: **Goertzel incremental sobre ventana de 10 ciclos
(5120 muestras a 25600 Hz) con 49 frecuencias de subgrupo interarmónico.**

## Implementación

| Componente | Archivo |
|------------|---------|
| `InterharmonicAccumulator` | `metrology_insight/src/harmonics.rs` |
| Campo en señal | `MetrologyInsightSignal.interharmonics: [f32; 49]` |
| Integración en pipeline | `processing.rs` (push tras process_signal de tensión) |
| Snapshot firmware | `CycleSnapshot.v_interharmonics` en `firmware/src/libraries/metrology/dsp.rs` |
| Consola | `cmd_metro.rs` muestra grupos 1..10 |

## Consumo de recursos

| Recurso | Por acumulador | 3 fases |
|---------|---------------|---------|
| Heap (estado Goertzel) | 588 B (3 × 49 × f32) | ~1.8 KB |
| Heap (coeficientes) | 196 B (49 × f32) | compartido |
| CPU (push_cycle) | 25 k iteraciones (512 × 49 filtros) | 3.75 M ops/s |
| CPU (% @ 240 MHz) | ~9 % por fase | ~27 % total |
