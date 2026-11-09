# zhex 0.1.0：首次发布与第二轮加速报告

日期：2026-11-10（Asia/Taipei）。

**zhex 0.1.0 已真实发布到 crates.io。** 发布步骤在 2026-11-10
01:33:16（UTC+8）输出 `Published zhex v0.1.0 at registry crates-io`；
GitHub Release 随后于 01:33:25 创建。首次发布使用完整实现，没有发布占名空壳。

- [crates.io 0.1.0](https://crates.io/crates/zhex/0.1.0)
- [GitHub Release v0.1.0](https://github.com/LIghtJUNction/zhex/releases/tag/v0.1.0)
- [实际发布工作流：16/16 作业成功](https://github.com/LIghtJUNction/zhex/actions/runs/42665401958)
- [发布状态与必要日志](validation/release-0.1.0/results.json)
- [公开下载包的独立审计](validation/release-0.1.0/artifact-audit.json)

```toml
[dependencies]
zhex = "0.1.0"
```

## 已交付的实现

发布 crate 始终为 `#![no_std]`，仅使用 `core`。普通、开发、构建与
target-specific 第三方依赖均为零，没有 `alloc`、`std`、C/FFI、外部运行时或
`build.rs`。最低支持版本为实际测试过的 Rust 1.70.0。

公开接口为 `encode_lower`、`encode_upper`、`decode`、`encoded_len` 和
`decoded_len`。调用方提供输出切片，转换不分配堆内存，成功时返回实际写入长度。
超长输出缓冲区的未使用后缀保持不变。

解码只接受 ASCII 十六进制字符，支持大小写混合。错误优先级固定为奇数长度、
输出容量不足、首个非法字节；非法字节错误使用原始输入的位置。预先可判断的
长度和容量错误不修改输出。非法字符之前的输出允许已经被修改，避免为事务性
写入增加一次全量预扫描。标量、SSE2、AVX2 和 NEON 保持相同结果和错误语义。

## 这次加速解决了什么

十六进制编码把一个字节分成两个四位值，分别转换成字符；解码反过来，并必须
检查每个字符是否合法。优化的作用是减少完成这些必要工作所需的指令和尾部处理。

本轮采用三个变化。首先，支持 AVX2 的 x86 平台从 16 字节开始使用向量编码，
较长输入保留原有 32 字节主循环。其次，SSE2 处理短解码，AVX2 用两个小表同时
完成字符分类与数值映射，并在写入前检查整个 64 字符分组。最后，不足一个向量的
尾部使用恰好结束于切片末端的完整向量，重复处理一小段已完成的前缀。

这种尾部处理的全部读写仍在输入和输出切片内部。它不需要 padding；解码重叠的
前缀此前已经通过验证，因此错误位置仍是原始输入中的第一个非法字节。安全入口
保留 CPU 与操作系统特性检查和标量回退。双表算法注明了 Daniel Lemire 的原始
公共领域来源，归属保留在源码与 [LICENSE](LICENSE) 中。

SSE2 编码、全程 widening 编码、128 字符算术解码批次以及显式指针循环没有获得
稳定收益，均未进入最终生产实现。实验源码、反汇编和失败结果全部保留。

## 实测收益和仍未达到的目标

测量环境为 Intel Xeon Platinum 8573C、Linux 6.18.57、Rust 1.99.1、LLVM
23.1.1。进程固定在逻辑 CPU 0，使用预分配的热缓冲区。generic 与
`-C target-cpu=native` 分别编译和测量。下表为几何平均加速比，分母为当前
zhex 耗时，数值大于 1 表示当前版本更快。

| 比较范围 | Generic | Native |
| --- | ---: | ---: |
| 相对上一版：0–64 字节，小写编码 | 1.278× | 1.386× |
| 相对上一版：0–64 字节，大写编码 | 1.301× | 1.360× |
| 相对上一版：0–64 字节，混合大小写解码 | 1.305× | 1.386× |
| 相对上一版：固定 96 个主要案例 | 1.012× | 1.055× |
| 相对每个案例的最强外部竞品：同一组 96 案例 | 0.783× | 0.794× |

**相对最强竞品达到 2× 的目标仍未达成。** 两种构建均为 0/96 主要案例达到
该目标。最强基线从 hex、faster-hex、const-hex、hex-simd、better-hex、fashex
和 hex-turbo 的语义对齐接口中逐格选择；旧版 zhex 和内部实验不会参与这个评分。

短输入的整体收益不能推广为每个长度、对齐和操作都更快。相对上一版，主要案例
中 generic 有 39/96 回退，native 有 40/96 回退。generic 主要案例的小写编码
平均为旧版的 0.934×；首字节非法的解码平均为 0.863×。这些结果和原始样本没有
删除，也没有从主要评分中移除。

固定主要输入仍为 16、32、64、256 字节、4 KiB、64 KiB、1 MiB、8 MiB，
三种操作和四组输入/输出偏移。完整覆盖另含每个 0–64 字节长度、向量边界和
非法字符位置。所有实现的输入生成、分配、预热和结果校验均按相同协议进行，
使用固定种子、`black_box` 和每组合七个未筛选样本。

本轮共保留 218,680 个计时样本和 656 条明确的 unsupported 记录。其中四组
冻结源码的最终实验提供 158,200 个计时样本，独立审计重新计算覆盖、吞吐率、
中位数、自比较和竞品评分，并核对源码快照及可执行文件哈希。吞吐率统一使用
payload GiB/s：编码的原始字节数、解码的输出字节数。

这些共享云主机数据属于探索性测量。没有独立证据证明已达到内存带宽上限；
没有宣称细小差异具有统计显著性，也没有把 ARM smoke 计时当作 ARM 性能结论。
详细延迟、吞吐率、回退、标量/SIMD/分派消融和复现命令见
[完整加速报告](comparison/ACCELERATION.md) 与
[独立性能审计](validation/acceleration-audit.json)。

## CI 与实际发布验证

发布工作流先解析 `v0.1.0` 的确定 commit，然后在该 commit 上完成全部必要检查：

- stable 与 Rust 1.70.0 在真实 x86-64 和 AArch64 runner 上执行 debug、release、
  doctest、native、强制标量与硬件支持的 SIMD 测试。
- 独立参考测试覆盖全部单字节、65,536 字节对、65,536 文本字节对、非法字符
  位置、容量差一字节、长度溢出、切片偏移、向量边界与大输入；SSE2 有直接后端测试。
- rustfmt、Clippy、标量 Miri、stable/MSRV 裸机 `thumbv7em-none-eabi` 编译检查。
- 零依赖和包内容检查，以及 `cargo package --list`、`cargo package --locked`
  和 `cargo publish --dry-run --locked`。发布包仅包含 13 个允许的文件。
- x86-64 与 AArch64 benchmark smoke 仅验证功能，不以共享 runner 的细微计时
  波动作硬性性能门槛。发布校验脚本另有 19 项隔离仓库测试。

这些检查通过后，工作流实际执行 `cargo +stable publish --locked`。注册表 token
只设置在这个步骤的环境中，没有执行 `cargo login`，没有把 token 放入命令参数、
文件或日志。GitHub Release 作业依赖实际发布成功，公开时间也在 crates.io 上传之后。

| 证据角色 | 完整 commit |
| --- | --- |
| 冻结性能源码 | `f69459e8e58db7d761099a00bb161da34ef638ee` |
| 完整文档与实验记录 | `9e1d46e2f14623dcb3d88be510b6699a4a994450` |
| 独立发布请求 | `4629474a37a32913721c90dfafe8b45f02c7515d` |
| 实际发布标签与 crate 源码提交 | `499fb9c5be2e292a7cf6d41d60b0d431f186f50f` |

发布前只增加了文档、实验资料和 comparison 的精确 path 版本要求；生产 `src/`
与冻结性能源码一致。正式包排除 comparison 工作区及其全部第三方依赖。

发布后还从官方 sparse index 读取版本记录并下载实际 `.crate`。下载文件的 SHA-256
为 `431a37454331a707d47b4060f8883100c2eb4fdce7d821dd2924f97e987953a8`，
与 index 完全一致。包内 VCS commit 与上表一致，13 个文件清单和相关源码逐字节
对应发布提交，index 的 `deps` 为空。

独立 `#![no_std]` consumer 使用下载解包后的源码，在 Rust 1.70.0 和 stable
1.99.1 上分别完成 default 与 `force-scalar` 的四次 release、locked、offline
编译，均通过。Cargo 1.70 对新版 Cargo 生成的 `package.autolib` 元数据给出
unused-key 提示，未阻止编译；原始提示保留在审计日志中。这项本机检查的目标为
x86-64 Linux，ARM 实机和裸机验证来自前述明确区分的 CI 作业。

## 后续如何 bump 版本

在 GitHub Actions 中选择 **Bump and release**，使用默认分支，选择 `patch`、
`minor`、`major`，或选择 `custom` 并填写明确的 `X.Y.Z`。例如下一次补丁发布：

```sh
gh workflow run bump.yml -R LIghtJUNction/zhex -f bump=patch
```

自定义版本示例：

```sh
gh workflow run bump.yml -R LIghtJUNction/zhex -f bump=custom -f version=0.2.0
```

工作流会同步两个 manifest 和两份锁文件，创建版本提交，原子推送新标签，再直接
调用完整发布工作流。版本必须递增，已有标签和已发布版本不能覆盖。首次使用的
`current` 只适用于尚无发布标签的 manifest 版本；现在再次选择它会明确失败。

也支持直接推送符合版本的 `vX.Y.Z` 标签，或在 **Release** 工作流中手动选择一个
已存在的标签。工作流检查标签、版本、确定 commit 和默认分支历史；发布串行执行，
不会中途取消。PR 和 fork 无法进入发布流程，不使用 `pull_request_target`。
缺少 `CARGO_REGISTRY_TOKEN` Secret 时会明确失败，不会创建虚假的 GitHub Release。

## 交付入口

项目、分阶段提交、CI 和完整证据位于
[LIghtJUNction/zhex](https://github.com/LIghtJUNction/zhex)。
[README](README.md) 保持精简，性能结论限定到实际测量硬件和输入范围。
[comparison/README.md](comparison/README.md) 提供完整复现命令，
[comparison/PLAN.md](comparison/PLAN.md) 保留预先固定的工作负载与统计口径。

本报告与完整 Git 源码/证据归档的 Google 云盘备份位置见本次交付回复。
