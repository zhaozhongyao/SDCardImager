import AppKit

let W: CGFloat = 1024
let SCALE: CGFloat = 0.80
let S: CGFloat = W * SCALE
let R: CGFloat = 185 * SCALE
let OFF = (W - S) / 2

let image = NSImage(size: NSSize(width: W, height: W))
image.lockFocus()

// 圆角方块（缩小 20%，居中留白）
let path = NSBezierPath(
  roundedRect: NSRect(x: OFF, y: OFF, width: S, height: S),
  xRadius: R,
  yRadius: R
)

// 对角渐变（左上 blue-500 #2563eb -> 右下 indigo-600 #4f46e5），与程序内 header 一致
let c1 = NSColor(calibratedRed: 0x25 / 255.0, green: 0x63 / 255.0, blue: 0xeb / 255.0, alpha: 1)
let c2 = NSColor(calibratedRed: 0x4f / 255.0, green: 0x46 / 255.0, blue: 0xe5 / 255.0, alpha: 1)
let gradient = NSGradient(colors: [c1, c2])
gradient?.draw(in: path, angle: 135)

// 白色粗体 "SD"，系统字体（与程序内 header 文字同源）
let font = NSFont.systemFont(ofSize: 500 * SCALE, weight: .bold)
let attrs: [NSAttributedString.Key: Any] = [
  .font: font,
  .foregroundColor: NSColor.white,
]
let str = NSAttributedString(string: "SD", attributes: attrs)
let textSize = str.size()
let origin = NSPoint(
  x: OFF + (S - textSize.width) / 2,
  y: OFF + (S - textSize.height) / 2 - 10 * SCALE
)
str.draw(at: origin)

image.unlockFocus()

guard
  let tiff = image.tiffRepresentation,
  let rep = NSBitmapImageRep(data: tiff),
  let png = rep.representation(using: .png, properties: [:])
else {
  print("render failed")
  exit(1)
}

let fm = FileManager.default
try? fm.createDirectory(atPath: "src-tauri/icons", withIntermediateDirectories: true)
try! png.write(to: URL(fileURLWithPath: "src-tauri/icons/app-icon.png"))
print("icon written: \(png.count) bytes")

