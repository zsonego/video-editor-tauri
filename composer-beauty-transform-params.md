# Composer 美颜预览视频变换参数约定

## 1. 适用接口

本文档适用于以下两个接口的 `json_params` 参数：

```c
int composer_beauty_process_frame(const char* input_video_path,
                                  int64_t timestamp_ms,
                                  const char* output_image_path,
                                  const char* json_params);

int composer_beauty_process_file(const char* input_path,
                                 const char* output_path,
                                 int64_t start_time_ms,
                                 int64_t duration_ms,
                                 const char* json_params);
```

图片预览和视频预览必须使用相同的变换规则。

## 2. 完整 JSON 示例

```json
{
  "whiteness": 0.5,
  "smoothing": 0.3,
  "skin_tone": 0.0,
  "face_detect": 1,
  "rotation": 30.0,
  "lut_file": "/absolute/path/to/lut.cube",
  "lut_intensity": 0.8,
  "positionX": 480.0,
  "positionY": 270.0,
  "scale": 0.5,
  "canvas_width": 960,
  "canvas_height": 540,
  "transform_origin": "center",
  "rotation_direction": "clockwise",
  "stabilization": false,
  "one_click_beauty": false
}
```

字段名区分大小写。`positionX` 和 `positionY` 使用驼峰命名，其他新增字段使用下划线命名。

## 3. 变换字段

| 字段 | JSON 类型 | 当前值或范围 | 说明 |
| --- | --- | --- | --- |
| `positionX` | number | 不限制 | 视频中心点在前端预览画布中的 X 坐标，单位为预览画布像素 |
| `positionY` | number | 不限制 | 视频中心点在前端预览画布中的 Y 坐标，单位为预览画布像素 |
| `scale` | number | `0.01`～`10.0` | Fabric 视频对象相对于源视频像素的等比缩放系数，不是百分数 |
| `rotation` | number | `0`～`360` | 围绕视频中心旋转的角度，单位为度 |
| `canvas_width` | integer | 当前固定为 `960` | 产生 `positionX`、`scale` 等数据的前端预览画布宽度 |
| `canvas_height` | integer | 当前固定为 `540` | 产生 `positionY`、`scale` 等数据的前端预览画布高度 |
| `transform_origin` | string | 当前固定为 `center` | 位置、缩放和旋转均以源视频中心为变换原点 |
| `rotation_direction` | string | 当前固定为 `clockwise` | `rotation` 正值表示顺时针旋转 |

前端属性面板中显示的缩放百分数等于 `scale * 100`。例如面板显示 `50%` 时，JSON 中的 `scale` 为 `0.5`。

`positionX`、`positionY` 允许超出画布范围或为负数，以支持把视频的一部分移动到画布外。

## 4. 坐标系

前端使用左上角为画布原点：

```text
(0, 0) --------------------> X
  |
  |
  |
  v
  Y
```

`positionX`、`positionY` 指向视频中心，不是视频左上角。默认居中位置为：

```text
positionX = canvas_width / 2
positionY = canvas_height / 2
```

当前默认值为 `(480, 270)`。

## 5. C++ 还原公式

设：

- 前端预览画布尺寸为 `canvas_width × canvas_height`。
- C++ 最终输出画布尺寸为 `output_width × output_height`。
- 解码后的源视频尺寸为 `source_width × source_height`。

先计算坐标换算比例：

```text
ratio_x = output_width  / canvas_width
ratio_y = output_height / canvas_height
```

前端和 C++ 输出画布宽高比一致时，`ratio_x` 应等于 `ratio_y`。此时：

```text
center_x = positionX * ratio_x
center_y = positionY * ratio_y
render_scale = scale * ratio_x
```

变换后的源视频显示尺寸为：

```text
render_width  = source_width  * render_scale
render_height = source_height * render_scale
```

必须按照以下顺序应用变换：

```text
1. 将源视频中心移动到局部坐标原点
2. 按 render_scale 等比缩放
3. 围绕中心按 rotation 顺时针旋转
4. 将旋转后的中心移动到 center_x、center_y
5. 裁剪到 output_width × output_height 输出画布
```

如果使用的图像库规定正角度为逆时针，例如常见的 OpenCV 旋转接口，则传给该接口的角度应取负数：

```cpp
double library_angle = -json_rotation;
```

## 6. 计算示例

输入视频和 C++ 输出画布均为 `1920×1080`，前端画布为 `960×540`：

```json
{
  "positionX": 480,
  "positionY": 270,
  "scale": 0.5,
  "rotation": 30,
  "canvas_width": 960,
  "canvas_height": 540,
  "transform_origin": "center",
  "rotation_direction": "clockwise"
}
```

换算结果：

```text
ratio_x = 1920 / 960 = 2
ratio_y = 1080 / 540 = 2

center_x = 480 * 2 = 960
center_y = 270 * 2 = 540
render_scale = 0.5 * 2 = 1.0
```

因此 C++ 应把源视频保持原始尺寸，以 `(960, 540)` 为中心，顺时针旋转 30 度，然后裁剪到 `1920×1080`。

## 7. 输出宽高比要求

要和前端预览完全一致，C++ 输出画布宽高比必须与 `canvas_width / canvas_height` 一致。当前为 `16:9`。

如果输出画布不是 `16:9`，`ratio_x` 和 `ratio_y` 会不同。此时不能同时保持等比缩放和完全一致的 X/Y 映射，C++ 必须与前端另行约定裁剪或留黑规则。当前版本不支持这种情况。

源视频宽高比可以不是 `16:9`；只要求最终输出画布与前端预览画布宽高比一致。

## 8. 默认值与兼容处理

Tauri 层对缺失参数使用以下默认值：

```json
{
  "positionX": 0,
  "positionY": 0,
  "scale": 1,
  "rotation": 0,
  "canvas_width": 960,
  "canvas_height": 540,
  "transform_origin": "center",
  "rotation_direction": "clockwise"
}
```

其中 `transform_origin` 当前只接受 `center`，`rotation_direction` 当前只接受 `clockwise`。C++ 可以按固定值实现，但仍应读取字段并对不支持的值返回参数错误，避免静默生成错误画面。

`source_width` 和 `source_height` 不通过 JSON 传递，由 C++ 在解码输入视频后获取。

