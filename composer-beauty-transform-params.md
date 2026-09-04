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
  "saturation": 100.0,
  "skin_tone": 0.0,
  "face_detect": 1,
  "rotation": 30.0,
  "lut_file": "/absolute/path/to/lut.cube",
  "lut_intensity": 0.8,
  "positionX": 960.0,
  "positionY": 540.0,
  "scale": 1.0,
  "canvas_width": 1920,
  "canvas_height": 1080,
  "transform_origin": "center",
  "stabilization": false,
  "one_click_beauty": false
}
```

字段名区分大小写。`positionX` 和 `positionY` 使用驼峰命名，其他新增字段使用下划线命名。

`saturation` 的取值范围为 `0`～`200`，其中 `100` 表示保持原始饱和度，`0` 表示完全去色，超过 `100` 表示增强饱和度。

## 3. 变换字段

| 字段 | JSON 类型 | 当前值或范围 | 说明 |
| --- | --- | --- | --- |
| `positionX` | number | 不限制 | 视频中心点在接口输出画布中的 X 坐标，单位为像素 |
| `positionY` | number | 不限制 | 视频中心点在接口输出画布中的 Y 坐标，单位为像素 |
| `scale` | number | `0.01`～`10.0` | 相对于视频默认铺满画布状态的缩放倍数；`1.0` 表示 `100%` |
| `rotation` | number | 不限制 | 围绕视频中心旋转的累计角度，单位为度；正数表示顺时针，负数表示逆时针，可超过一圈 |
| `canvas_width` | integer | 当前固定为 `1920` | C++ 预览接口的输出画布宽度 |
| `canvas_height` | integer | 当前固定为 `1080` | C++ 预览接口的输出画布高度 |
| `transform_origin` | string | 当前固定为 `center` | 位置、缩放和旋转均以源视频中心为变换原点 |

属性面板显示的缩放百分数等于 `scale * 100`。视频首次载入时会先在编辑器内部铺满画布，但这部分适配比例不计入 `scale`，所以用户没有调整时始终传 `1.0（100%）`。

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

当前接口默认值为 `(960, 540)`。

## 5. C++ 还原公式

设：

- JSON 约定画布尺寸为 `canvas_width × canvas_height`。
- C++ 最终输出画布尺寸为 `output_width × output_height`。
- 解码后的源视频尺寸为 `source_width × source_height`。

如果最终输出尺寸与 JSON 画布尺寸不同，先计算坐标换算比例：

```text
ratio_x = output_width  / canvas_width
ratio_y = output_height / canvas_height
```

前端和 C++ 输出画布宽高比一致时，`ratio_x` 应等于 `ratio_y`。视频默认铺满画布的基础缩放为：

```text
center_x = positionX * ratio_x
center_y = positionY * ratio_y
base_scale = max(output_width / source_width, output_height / source_height)
render_scale = base_scale * scale
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
3. 围绕中心按 rotation 旋转：正值顺时针，负值逆时针
4. 将旋转后的中心移动到 center_x、center_y
5. 裁剪到 output_width × output_height 输出画布
```

如果使用的图像库规定正角度为逆时针，例如常见的 OpenCV 旋转接口，则传给该接口的角度应取负数：

```cpp
double library_angle = -json_rotation;
```

## 6. 计算示例

输入视频和 C++ 输出画布均为 `1920×1080`，用户没有调整缩放：

```json
{
  "positionX": 960,
  "positionY": 540,
  "scale": 1.0,
  "rotation": 30,
  "canvas_width": 1920,
  "canvas_height": 1080,
  "transform_origin": "center"
}
```

换算结果：

```text
ratio_x = 1920 / 1920 = 1
ratio_y = 1080 / 1080 = 1

center_x = 960 * 1 = 960
center_y = 540 * 1 = 540
base_scale = max(1920 / 1920, 1080 / 1080) = 1.0
render_scale = 1.0 * 1.0 = 1.0
```

因此 C++ 应把源视频保持原始尺寸，以 `(960, 540)` 为中心，顺时针旋转 30 度，然后裁剪到 `1920×1080`。

## 7. 输出宽高比要求

要和前端预览完全一致，C++ 输出画布宽高比必须与 `canvas_width / canvas_height` 一致。当前为 `16:9`。

如果输出画布不是 `16:9`，`ratio_x` 和 `ratio_y` 会不同。此时不能同时保持等比缩放和完全一致的 X/Y 映射，C++ 必须与前端另行约定裁剪或留黑规则。当前版本不支持这种情况。

源视频宽高比可以不是 `16:9`；只要求最终输出画布与 JSON 约定画布宽高比一致。

## 8. 默认值与兼容处理

Tauri 层对缺失参数使用以下默认值：

```json
{
  "positionX": 0,
  "positionY": 0,
  "scale": 1,
  "rotation": 0,
  "canvas_width": 1920,
  "canvas_height": 1080,
  "transform_origin": "center"
}
```

其中 `transform_origin` 当前只接受 `center`。`rotation` 保存用户的累计圈数，例如 `900` 表示顺时针两圈半，`-540` 表示逆时针一圈半。C++ 根据 `rotation` 的正负判断方向，渲染时可以使用与其等效的模 360 角度，但不得把归一化结果当成持久化值回写。

前端将 `rotation` 保留到小数点后两位。

`source_width` 和 `source_height` 不通过 JSON 传递，由 C++ 在解码输入视频后获取。

工程 XML 与图片、视频预览接口统一保存 `1920×1080` 画布坐标。前端编辑器内部仍使用 `960×540` 画布，读取和保存时会自动换算位置；内部铺满画布产生的基础缩放不会写入 `scale`。

## 9. 工程模板 XML 中的属性

用户调整属性后，应用会根据当前素材的 `asset id`，更新当前工程 `template.xml` 中所有 `asset-id` 相同的 `area`。`property` 位于 `area` 的最后，重复调整时更新原节点，不会重复追加：

```xml
<area id="area-id" asset-id="asset-id">
    <!-- area 原有节点 -->
    <property>
        <whiteness>0.0</whiteness>
        <smoothing>0.0</smoothing>
        <saturation>122.0</saturation>
        <skin_tone>0.0</skin_tone>
        <face_detect>1</face_detect>
        <rotation>0.0</rotation>
        <lut_style>/应用资源目录/luts/自然清新质感.cube</lut_style>
        <lut_intensity>0.0</lut_intensity>
        <positionX>960.0</positionX>
        <positionY>540.0</positionY>
        <scale>1.0</scale>
        <canvas_width>1920</canvas_width>
        <canvas_height>1080</canvas_height>
        <transform_origin>center</transform_origin>
        <stabilization>false</stabilization>
        <one_click_beauty>false</one_click_beauty>
    </property>
</area>
```

`lut_style` 保存当前应用资源目录中 LUT 文件的绝对路径，与图片、视频预览 JSON 的 `lut_file` 完全一致；未选择 LUT 时保存 `none`。页面重新打开模板时根据路径中的 LUT 文件名恢复下拉框。C++ 读取模板时可直接使用 `lut_style`，预览接口仍读取 JSON 中的 `lut_file` 和 `lut_intensity`。
