# Composer C++ 接口文档

## 概述

本文档说明 Composer 合成库对外提供的 C++ 接口。

调用约定：

- 使用合成相关接口前，必须先调用 `composer_init()` 初始化合成库及底层 MLT 工厂。
- 程序退出前，应调用 `composer_cleanup()` 释放全局资源。
- 合成操作使用一站式接口 `composer_compose()` 完成。

## 类型定义

### ComposerErrorCode

```cpp
typedef enum {
    COMPOSER_SUCCESS = 0,
    COMPOSER_ERROR_INVALID_XML = -1,
    COMPOSER_ERROR_FILE_NOT_FOUND = -2,
    COMPOSER_ERROR_MLT_INIT = -3,
    COMPOSER_ERROR_COMPOSITION_FAILED = -4,
    COMPOSER_ERROR_CANCELLED = -5
} ComposerErrorCode;
```

错误码定义。

| 错误码 | 值 | 说明 |
| --- | --- | --- |
| `COMPOSER_SUCCESS` | `0` | 调用成功 |
| `COMPOSER_ERROR_INVALID_XML` | `-1` | XML 文件无效 |
| `COMPOSER_ERROR_FILE_NOT_FOUND` | `-2` | 文件未找到 |
| `COMPOSER_ERROR_MLT_INIT` | `-3` | MLT 工厂初始化失败 |
| `COMPOSER_ERROR_COMPOSITION_FAILED` | `-4` | 合成失败 |
| `COMPOSER_ERROR_CANCELLED` | `-5` | 合成被取消 |

### ProgressCallback

```cpp
typedef void (*ProgressCallback)(int percent, const char* message, void* userdata);
```

进度回调函数类型定义。

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `percent` | `int` | 当前进度百分比，范围为 `0-100` |
| `message` | `const char*` | 当前处理阶段的描述信息，目前为固定的 `处理中...` |
| `userdata` | `void*` | 用户传入的自定义数据指针，暂无用 |

### CompositionHandle

```cpp
typedef void* CompositionHandle;
```

合成上下文句柄类型。

当前接口用于表示合成上下文句柄。

## 接口列表

| 接口 | 说明 |
| --- | --- |
| `composer_init` | 初始化合成库及底层 MLT 工厂 |
| `composer_cleanup` | 清理合成库占用的全局资源 |
| `composer_compose` | 一站式执行视频合成 |

---

## composer_init

```cpp
COMPOSER_API int composer_init(void);
```

初始化合成库及底层的 MLT 工厂。

必须在调用其他 `composer_*` 接口之前调用。

### 返回值

| 返回值 | 说明 |
| --- | --- |
| `COMPOSER_SUCCESS` | 初始化成功 |
| `COMPOSER_ERROR_MLT_INIT` | 初始化失败 |

---

## composer_cleanup

```cpp
COMPOSER_API void composer_cleanup(void);
```

清理合成库占用的全局资源。

程序退出前应调用一次以释放资源。

## composer_compose

```cpp
COMPOSER_API int composer_compose(
    const char* template_path,
    const char* project_path,
    const char* output_path,
    ProgressCallback callback,
    void* userdata
);
```

一站式合成接口，合并创建合成上下文、设置输出文件、执行合成渲染的功能。

### 参数

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `template_path` | `const char*` | 模板 XML 文件路径 |
| `project_path` | `const char*` | 工程 XML 文件路径 |
| `output_path` | `const char*` | 输出视频文件路径 |
| `callback` | `ProgressCallback` | 可选的进度回调函数，可以为 `NULL` |
| `userdata` | `void*` | 传递给回调的用户数据指针 |

### 返回值

| 返回值 | 说明 |
| --- | --- |
| `COMPOSER_SUCCESS` | 合成成功 |
| 其他错误码 | 合成失败，具体错误码见 `ComposerErrorCode` |

## 典型调用流程

```cpp
void on_progress(int percent, const char* message, void* userdata) {
    // percent: 0-100
    // message: 当前处理阶段的描述信息
}

int result = composer_init();
if (result != COMPOSER_SUCCESS) {
    return result;
}

result = composer_compose(
    "template.xml",
    "project.xml",
    "output.mp4",
    on_progress,
    NULL
);

if (result != COMPOSER_SUCCESS) {
    composer_cleanup();
    return result;
}

composer_cleanup();
return result;
```
