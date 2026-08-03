import numpy as np
from scipy import signal

fs = 8000.0

# 1. 6th Order Butterworth LPF at 35Hz
fc = 35.0
b_bw, a_bw = signal.butter(6, fc, 'low', fs=fs)

# 2. IEC Weighting Filter (230V, 50Hz)
k = 1.74802
lam = 2 * np.pi * 4.05981
w1 = 2 * np.pi * 9.15494
w2 = 2 * np.pi * 2.27979
w3 = 2 * np.pi * 1.22535
w4 = 2 * np.pi * 21.9

# Numerator: k * w1 * s * (1 + s/w2) = (k*w1/w2) * s^2 + (k*w1) * s
num = [k * w1 / w2, k * w1, 0]

# Denominator: (s^2 + 2*lam*s + w1^2) * (1 + s/w3) * (1 + s/w4)
# (s^2 + 2*lam*s + w1^2) * ( (1/(w3*w4))*s^2 + (1/w3 + 1/w4)*s + 1 )
p1 = [1, 2*lam, w1**2]
p2 = [1/(w3*w4), 1/w3 + 1/w4, 1]
den = np.convolve(p1, p2)

# Convert Weighting filter to digital
system = (num, den)
b_wt, a_wt = signal.bilinear(num, den, fs=fs)

print("Butterworth 6th order LPF (35Hz) at fs=8000Hz:")
print("b_bw =", list(b_bw))
print("a_bw =", list(a_bw))

print("\nIEC Weighting Filter (230V 50Hz) at fs=8000Hz:")
print("b_wt =", list(b_wt))
print("a_wt =", list(a_wt))

