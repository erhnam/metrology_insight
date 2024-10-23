import numpy as np
import pandas as pd
import matplotlib.pyplot as plt

# Señal ADC real
signal_adc = np.array([
26, 23, 20, 19, 16, 13, 12, 10, 8, 7, 6, 5, 4, 3, 3, 2, 2, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 6, 3, 10, 12, 12, 19, 22, 25, 28, 32, 36, 38, 42, 46, 49, 52, 54, 55, 60, 62, 66, 67, 70, 75, 76, 79, 81, 83, 86, 87, 88, 91, 93, 95, 97, 97, 97, 98, 98, 95, 99, 99, 94, 97, 93, 94, 95, 94, 93, 92, 91, 90, 84, 87, 83, 77, 78, 70, 72, 70, 66, 65, 62, 59, 57,40, 37, 33, 34, 26, 28, 26, 23, 19, 19, 15, 13, 11, 9, 8, 7, 6, 5, 4, 3, 3, 2, 2, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 4, 5, 8, 11, 14, 18, 23, 24, 29, 31, 35, 38, 40, 44, 45, 50, 54, 56, 60, 62, 66, 68, 69, 76, 76, 79, 81, 83, 86, 88, 89, 91, 92, 95, 95, 96, 97, 98, 97, 98, 98, 98, 100, 101, 98, 94, 98, 97, 90, 93, 89, 90, 86, 82, 83, 75, 77, 76, 71, 71, 69, 66, 65, 61, 60,31, 36, 35, 32, 32, 27, 27, 25, 23, 18, 16, 14, 12, 10, 9, 7, 6, 5, 4, 3, 3, 2, 2, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 5, 7, 9, 14, 16, 16, 23, 23, 29, 33, 35, 41, 42, 45, 49, 52, 55, 59, 61, 65, 68, 70, 74, 77, 81, 83, 84, 88, 87, 88, 91, 93, 91, 95, 96, 95, 97, 97, 93, 98, 95, 95, 97, 95, 95, 96, 95, 96, 95, 94, 88, 86, 86, 84, 82, 82, 78, 76, 72, 70, 68, 66, 64, 61,42, 40, 40, 35, 36, 33, 27, 28, 23, 19, 18, 15, 13, 11
])

# Parámetros del sistema
vref = 1.8
adc_max = 4095  # 4095
offset = 1.65  # V

# Convertir a voltios
signal_volts = (signal_adc / adc_max) * vref

print("Media en voltios:", np.mean(signal_volts))
offset_real = np.mean(signal_volts)

# Centrar la señal quitando el offset
signal_centered = signal_volts - offset_real

# Paso 3: Convertir voltios a amperios (factor SCT013-030)
current_amperes = signal_centered * 30

# Detectar cruces por cero descendentes
zc_indices = np.where((current_amperes[:-1] >= 0) & (current_amperes[1:] < 0))[0] + 1

# Calcular fase si hay cruces suficientes
if len(zc_indices) >= 2:
    first_zc = zc_indices[0]
    second_zc = zc_indices[1]
    samples_per_cycle = second_zc - first_zc
    phase_deg = ((samples_per_cycle - (first_zc % samples_per_cycle)) / samples_per_cycle) * 360
    phase_deg = phase_deg % 360
else:
    first_zc = second_zc = samples_per_cycle = phase_deg = None

# Mostrar resultados
df = pd.DataFrame({
    'first_zc': [first_zc],
    'second_zc': [second_zc],
    'samples_per_cycle': [samples_per_cycle],
    'phase_deg': [phase_deg]
})
print(df)

# Gráfico
plt.figure(figsize=(10, 4))
plt.plot(current_amperes, label='Señal ADC')
if first_zc is not None and second_zc is not None:
    plt.axvline(first_zc, color='r', linestyle='--', label='1er cruce 0')
    plt.axvline(second_zc, color='g', linestyle='--', label='2º cruce 0')
plt.title('Cruces por cero - señal real ADC')
plt.xlabel('Muestra')
plt.ylabel('Valor ADC')
plt.grid(True)
plt.legend()
plt.tight_layout()
plt.show()
