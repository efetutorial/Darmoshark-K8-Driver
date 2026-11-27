# Blade Graphics 0.7.0

This is the source from the `blade-graphics` 0.7.0 crates.io release, built from
[upstream revision 896b1cc](https://github.com/kvark/blade/tree/896b1cc145b9ecb8e25032160356cb1e50e84296/blade-graphics).
It is distributed under the MIT license in this directory.

The Vulkan backend retains the active surface configuration and rebuilds the
swapchain when acquiring a frame on Wayland returns `VK_ERROR_OUT_OF_DATE_KHR`.
It retries acquisition once after rebuilding. Remove the Cargo patch when GPUI
uses a renderer that recovers from this condition.
