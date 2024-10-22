import numpy as np
import math

# Adapt following variables as needed
VPEAK = 325
IPEAK = 100
IPHASE = 0
#SAMPLES_OFFSET = 9128
SAMPLES_OFFSET = 0
NOISE_Freq = 6000
NOISE_VPEAK_PERCENT = 0
NOISE_IPEAK_PERCENT = 0
NOISE_RANDOM_PERCENT = 0

ZERO_CROSSING_MAX_POINTS = 5
FREQ_ZC_DEBOUNCE = 5

np.random.seed(0)
# Following variables are related to device FW values
Fs = 7812.5 # 8000
F = 50 # Hz

nSamples = 177 #same number used in FW #int((Fs/F)) # Fs/F --> nearest integer
VinToCounts = 9289.14
AmpsToCounts = 1048.5760

# Harmonics
ENABLE_HARMONICS = False
HARM_FREQ = F * 5
VHPEAK = VPEAK * 0.5
IHPEAK = IPEAK * 0.5
INTERPOLATION_FACTOR = 1.220703125

def offset( deg ):
    return deg*2*np.pi/360

def voltage( v ):
    return v*VinToCounts

def current( i ):
    return i*AmpsToCounts

def signalOffsetRemove(s):
    offset = int( (np.max(s) + np.min(s)) / 2 )
    signal = []
    # Remove offset from signal
    for i in range(len(s)):
        aux_sample = 0
        aux_sample = int(s[i])
        signal.append(int(aux_sample) - offset)

    signal = np.array(signal)

    return signal

def signalIntegrate(s, frequencyZC):
    integral = 0
    y_x = 0 # y(x)
    y_x1 = 0 # y(x+1)
    res_signal = []

    # In case the signal needs to be integrated (only rogwosky currents)
    orms = (signalRMS(s, frequencyZC, int(Fs/F))/AmpsToCounts)

    # Cummulative trepezoidal rule integration
    for i in range(len(s)):
        y_x = s[i]
        try:
            y_x1 = s[i+1]
        except:
            y_x1 = y_x
        integral += ((y_x + y_x1)/2)
        res_signal.append(int(integral))

    res_signal = np.array(res_signal)

    # Offset integrated signal: res_signal
    res_signal = signalOffsetRemove(res_signal)

    # Scale to 0db (higher frequencies are attenuated): res_signal
    integral_rms = (signalRMS(res_signal, frequencyZC, len(s)) / AmpsToCounts)

    int_k = 0.0
    if (orms != 0):
        int_k = integral_rms / orms
    else:
        int_k = 1

    aux_signal = 0
    for i in range(len(s)):
        aux_signal = int(res_signal[i] / int_k)
        res_signal[i] = aux_signal

    return (res_signal)

def signalFrequencyZC(buffer):
    numSamples = len(buffer)
    numCrossing = 0
    frequency = -1
    interpolationPoints = np.zeros(ZERO_CROSSING_MAX_POINTS)
    debounce = 0

    for p in range(numSamples - 1):
        if ((debounce == 0) and ((buffer[p] > 0 and buffer[p + 1] <= 0) or (buffer[p] < 0 and buffer[p + 1] >= 0))):
            x1 = p
            y1 = buffer[p]
            x2 = p+1
            y2 = buffer[p+1]
            yp = 0
            xp = x1 + (yp - y1) / ((y2 - y1) / (x2 - x1))

            if (numCrossing < ZERO_CROSSING_MAX_POINTS):
                interpolationPoints[numCrossing] = xp
                numCrossing += 1

            debounce = FREQ_ZC_DEBOUNCE

        if (debounce > 0):
            debounce -= 1

    if (numCrossing > 1):
        sum = 0
        for p in range(numCrossing - 1):
            sum += interpolationPoints[p+1] - interpolationPoints[p]

        cycleAvg = (sum / (numCrossing - 1)) * 2
        frequency = 1 / (cycleAvg / Fs)

    return frequency

def peak(array):
    return np.abs(array[0:int(Fs/F)]).max()

def signalRMS(signal, frequency, length):
    square = 0
    mean = 0
    rms = 0
    sample = 0
    n_length = length # Integer part
    d_length = 0; # Decimal part
    p_length = length # n + d length, fractional length of cycle
    ysample = 0; # Last interpolated y sample at fractinal x

    if (frequency > 0):
        d_length = math.modf(Fs / frequency)[0]
        n_length = int(math.modf(Fs / frequency)[1])
        p_length = n_length + d_length
    
    # Compute last interpolated sample
    if (d_length > 0): # Only interpolate frac sample if fractional part of cycle length exists.
        ysample = (((1-d_length)/2) * signal[int(n_length) - 1]) + (((1+d_length)/2) * signal[int(n_length)])

    # Compute RMS integer N part
    for i in range(n_length):
        sample = signal[i]
        square += math.pow(sample,2)

    square += (math.pow(ysample,2) * d_length)

    mean = (square / p_length)
    rms = math.sqrt(mean)

    return rms

# GENERATE SIGNALS
samples = np.arange(0, nSamples)

# NOISE SIGNALS
noise = np.random.random(size=nSamples)

noise = noise - noise.mean()

signal_noise_random = voltage(VPEAK) * (noise/noise.max()) * NOISE_RANDOM_PERCENT

signal_noise_v = voltage(VPEAK) * np.sin( offset(0) +(2 * np.pi*NOISE_Freq)/Fs * samples) * NOISE_VPEAK_PERCENT
signal_noise_i = current(IPEAK) * np.sin( offset(0) +(2 * np.pi*NOISE_Freq)/Fs * samples) * NOISE_IPEAK_PERCENT

# VOLTAGE
signal_v1 = voltage(VPEAK) * np.sin( offset(0) + (2 * np.pi*F)/Fs * samples) + signal_noise_v + signal_noise_random
signal_v2 = voltage(VPEAK) * np.sin( offset(120) + (2 * np.pi*F)/Fs * samples) + signal_noise_v + signal_noise_random
signal_v3 = voltage(VPEAK) * np.sin( offset(240) + (2 * np.pi*F)/Fs * samples) + signal_noise_v + signal_noise_random
signal_v4 = 0 * samples

# CURRENT
signal_i1 = current(IPEAK) * np.cos( offset(0+90) + offset(IPHASE) + (2 * np.pi*F)/Fs * samples) + signal_noise_i
signal_i2 = current(IPEAK) * np.cos( offset(120+90) + offset(IPHASE) + (2 * np.pi*F)/Fs * samples) + signal_noise_i
signal_i3 = current(IPEAK) * np.cos( offset(240+90) + offset(IPHASE) + (2 * np.pi*F)/Fs * samples) + signal_noise_i
signal_i4 = 0 * samples

# HARMONICS
if ENABLE_HARMONICS:
    signal_v1 += voltage(VHPEAK) * np.sin( offset(0) + (2 * np.pi*HARM_FREQ)/Fs * samples)
    signal_v2 += voltage(VHPEAK) * np.sin( offset(120) + (2 * np.pi*HARM_FREQ)/Fs * samples)
    signal_v3 += voltage(VHPEAK) * np.sin( offset(240) + (2 * np.pi*HARM_FREQ)/Fs * samples)

    signal_i1 += current(IHPEAK) * np.cos( offset(0+90) + offset(IPHASE) + (2 * np.pi*HARM_FREQ)/Fs * samples)
    signal_i2 += current(IHPEAK) * np.cos( offset(120+90) + offset(IPHASE) + (2 * np.pi*HARM_FREQ)/Fs * samples)
    signal_i3 += current(IHPEAK) * np.cos( offset(240+90) + offset(IPHASE) + (2 * np.pi*HARM_FREQ)/Fs * samples)

signals = [ signal_v1, signal_v2, signal_v3, signal_v4,
            signal_i1, signal_i2, signal_i3, signal_i4]
# ADD OFFSETS
signals = [x+SAMPLES_OFFSET for x in signals]

# Truncate to integers (match devcie precision)
signals = [np.trunc(x) for x in signals]

frequencyZC = []
for s in signals[0:4]:
    frequencyZC.append(signalFrequencyZC(s))

# Integrate Currents (match device algorithm)
integrated_signals = []
integrated_signals.append(signalIntegrate(signals[4], frequencyZC[4-4]))

for i in range(4,8):
    integrated_signals.append(signalIntegrate(signals[i], frequencyZC[i-4]))

signals = signals[0:4] + integrated_signals

print("\nVoltages")
for i in range(1):
    print("\t[peak: %f] " % (peak(signals[i]) / VinToCounts))
    print("\t[Fz: %f]" % frequencyZC[i])
    print("\t[rms: %f]" % (signalRMS(signals[i], frequencyZC[i], int(Fs/F)) / VinToCounts) )    
    print("")

print("\nCurrents")
for i in range(4,5):
    print("\t[peak: %f]" % (peak(signals[i]) / AmpsToCounts))
    print("\t[Fz: %f]" % frequencyZC[i-4])
    print("\t[rms: %f]" % (signalRMS(signals[i], frequencyZC[i-4], int(Fs/F)) / AmpsToCounts) )    
    print("")
