# Live OCR
Live OCR and character definitions on mouseover. Extremely WIP.

Press `Alt + X` to toggle, OCR is done once when toggling on. Click on paragraphs in the application window to copy them to your clipboard (i.e. to paste into a translator).

**Needs to be launched as admin to work in applications that also launch as admin (i.e. ZZZ).**

### Example Screenshots
| Tooltip | App Window |
| --- | --- |
| ![Screenshot of the hover over tooltip](/assets/example1.webp) | ![Screenshot of the application window allowing copying of entire paragraphs](/assets/example2.webp) |

# Support for non-Windows OS
This application should work on Linux/MacOS out of the box, but is not tested. Feel free to build from source.

# Building from source

1. Install cargo
2. Install Tauri CLI v1
3. Run `cargo tauri build` to create an installer bundle or `cargo tauri build -b none` to build a loose distribution

# Running from source

1. Install cargo
2. Install Tauri CLI v1
3. Install dependencies listed below
4. Run `cargo tauri dev --release` to run a dev server. Debug builds are too slow for OCR.

## With TensorRT

### Required dependencies
- CUDA 11.8 OR 12.x (tested with 12.1)
- CuDNN for corresponding CUDA version
- TensorRT 10 for corresponding CUDA version
- ONNX Runtime for corresponding CUDA version

#### Specific DLLs required in path or next to the binary

This is probably only relevant for bundling the application

    cublas64_12.dll  
    cublasLt64_12.dll  
    cudart64_12.dll  
    cudnn64_9.dll  
    cudnn_graph64_9.dll  
    cufft64_11.dll  
    nvinfer_10.dll  
    nvinfer_builder_resource_10.dll  
    nvinfer_plugin_10.dll  
    nvonnxparser_10.dll
    onnxruntime.dll  
    onnxruntime_providers_cuda.dll  
    onnxruntime_providers_shared.dll  
    onnxruntime_providers_tensorrt.dll  

## Without GPU Acceleration

### Required Dependencies

- ONNX Runtime