import numpy as np
from scipy import signal

fs = 8000.0

# 1. 6th Order Butterworth LPF at 35Hz
fc = 35.0
sos_bw = signal.butter(6, fc, 'low', fs=fs, output='sos')

# 2. IEC Weighting Filter (230V, 50Hz)
k = 1.74802
lam = 2 * np.pi * 4.05981
w1 = 2 * np.pi * 9.15494
w2 = 2 * np.pi * 2.27979
w3 = 2 * np.pi * 1.22535
w4 = 2 * np.pi * 21.9

num = [k * w1 / w2, k * w1, 0]
p1 = [1, 2*lam, w1**2]
p2 = [1/(w3*w4), 1/w3 + 1/w4, 1]
den = np.convolve(p1, p2)

sos_wt = signal.tf2sos(num, den)
# Note tf2sos is for continuous if not specified, but wait! We need to convert tf to digital FIRST, then get SOS.
# Actually, bilinear returns b, a. Then we can use tf2sos on the digital b, a!
b_wt, a_wt = signal.bilinear(num, den, fs=fs)
sos_wt_dig = signal.tf2sos(b_wt, a_wt)

def print_sos(name, sos):
    print(f"pub const {name}: [[f32; 6]; {len(sos)}] = [")
    for row in sos:
        print(f"    [{row[0]:.10e}, {row[1]:.10e}, {row[2]:.10e}, {row[3]:.10e}, {row[4]:.10e}, {row[5]:.10e}],")
    print("];")

print_sos("SOS_BW_35HZ", sos_bw)
print_sos("SOS_WEIGHTING", sos_wt_dig)

