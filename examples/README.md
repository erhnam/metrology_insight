
# Metrology Insight for Milk-V Duo

This project is an electrical metrology application designed to run on the **Milk-V Duo** board using the **riscv64** architecture. Below are the necessary steps to properly run the binary on the target embedded Linux system.

---

## 🧱 System Requirements

- Milk-V Duo board (with embedded Linux)
- Root access (to load kernel modules and create symbolic links)
- `metrology_insight` binary compiled for `riscv64gc-unknown-linux-musl`
- Compatible MUSL dynamic libraries

---

## ⚙️ Environment Setup

1. **Load the ADC kernel module** (SAR ADC driver for CV180X):

```bash
insmod /mnt/system/ko/cv180x_saradc.ko
```

A precompiled kernel module cv180x_saradc.ko is provided with this project. It is specifically configured to sample 156 ADC readings at an interval of 128 microseconds, which corresponds to one full 20 ms period (i.e. 50 Hz).
Copy this file to your Milk-V Duo and load it as shown above.

2. **Grant execution permission to the binary**:

```bash
chmod +x milk_v_duo
```

3. **Set up the required MUSL libraries**:
In some embedded environments, the MUSL dynamic linker is not correctly linked by default. You need to create symbolic links manually:

```bash
# Link the dynamic linker
ln -s ./lib/ld-musl-riscv64v0p7_xthead.so.1 /lib/ld-musl-riscv64.so.1

# Link the standard C library
ln -s ./usr/lib64v0p7_xthead/lp64d/libc.so /lib/libc.so
```

Ensure the linker has the correct permissions:

```bash
chmod 755 /lib/ld-musl-riscv64v0p7_xthead.so.1
ln -sf /lib/ld-musl-riscv64v0p7_xthead.so.1 /lib/ld-musl-riscv64.so.1
```

4. **Running the Application**:

```bash
RUST_LOG=info ./milk_v_duo
```

## 🛠️ Compilation

To compile the binary from a host machine (e.g., x86_64), use:

```bash
cargo +nightly build --release --target=riscv64gc-unknown-linux-musl -Zbuild-std=std,core
```

Make sure you have a properly configured rust-toolchain.toml.

## 📄 License

This project is licensed under terms defined by the author or your organization.