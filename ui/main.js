const electron = require('electron')
// Module to control application life.
const app = electron.app
// Module to create native browser window.
const BrowserWindow = electron.BrowserWindow

const path = require('path')
const url = require('url')

const ffi = require('ffi')
const ref = require('ref')

var rust = ffi.Library("../capi/target/debug/slink.dll", {
  'new': ['pointer', ['uint32'] ],
  'open_loopback': ['bool', ['pointer'] ],
  'close': ['void', ['pointer']],
  'tick' : ['bool', ['pointer', 'uint'] ],
  'send' : ['uint32', ['pointer', 'pointer', 'pointer', 'uint'] ],
  'release' : ['void', ['pointer'] ],
  'set_recv_callback' : ['void', ['pointer', 'pointer'] ],
  'set_ack_callback' : ['void', ['pointer', 'pointer'] ],
  'set_expire_callback' : ['void', ['pointer', 'pointer'] ],
  'set_retry_callback' : ['void', ['pointer', 'pointer'] ],
  'set_observe_callback' : ['void', ['pointer', 'pointer'] ],
  'str_to_addr' : ['uint32', ['string'] ],
  'addr_to_str' : ['void', ['uint32', 'pointer']]
})

var rust_serial = ffi.Library("../capi_serial/target/debug/slink_serial.dll", {
  'open_port': ['bool', ['pointer', 'string', 'uint'] ],
})

function addr_to_str(addr) {
  var buffer = Buffer.alloc(7)
  rust.addr_to_str(addr, buffer)

  var output = buffer.toString()

  //Trim any trailing zeros
  var trim = 0;

  while(output.charAt(output.length - trim - 1) == '0') {
    ++trim
  }

  if (trim == output.length) {
    return ""
  } else {
    return output.substr(0, output.length - trim)
  }
}

function route_to_arr(route) {
  var translatedRoute = []

  var splitIdx = -1
  for(var i = 0; i < 17; ++i) {
    let translated = addr_to_str(route.readInt32LE(i*4))

    if(splitIdx == -1 || translated.length > 0) {
      translatedRoute.push(translated)
    }

    if(translated.length == 0 && splitIdx == -1) {
      splitIdx = i
    }
  }

  return translatedRoute
}

// Keep a global reference of the window object, if you don't, the window will
// be closed automatically when the JavaScript object is garbage collected.
let mainWindow
let link
let tick
let last_update = Date.now();

//Pin the callback so we don't GC it
let recv_callback
let ack_callback
let obs_callback
let expire_callback
let retry_callback

electron.ipcMain.on('send', (event, msg) => {
  console.log("send")
  console.log(msg)

  let data = Buffer.from(msg.msg)
  let route = Buffer.alloc(4 * 15);
  for(i = 0; i < msg.route.length; ++i ) {
    route.writeInt32LE(rust.str_to_addr(msg.route[i]), i*4)
  }

  let prn = rust.send(link, route, data, data.length)

  let sent = {
    prn: prn,
    route: msg.route,
    msg: msg.msg
  }

  mainWindow.send('send', sent)
})

electron.ipcMain.on('init', (event, msg) => {
  console.log("init")

  let addr = rust.str_to_addr(msg.callsign)
  link = rust.new(addr)

  recv_callback = ffi.Callback('void', ['uint32*', 'uint32', 'char*', 'uint'],
    function(routePtr, prn, dataPtr, size) {
      let data = ref.reinterpret(dataPtr, size)
      let route = ref.reinterpret(routePtr, 17 * 4)

      let translatedRoute = route_to_arr(route)

      let msg = {
        'msg': data.toString(),
        'prn': prn,
        'route': translatedRoute
      }
      console.log("Recv ")
      console.log(msg)
      mainWindow.send('recv', msg)
    })
  rust.set_recv_callback(link, recv_callback)

  ack_callback = ffi.Callback('void', ['pointer', 'uint32'],
    function(routePtr, prn) {
      let route = ref.reinterpret(routePtr, 17 * 4)
      let translatedRoute = route_to_arr(route)

      let msg = {
        'prn': prn,
        'route': translatedRoute
      }
      console.log("Ack");
      console.log(msg)
      mainWindow.send('ack', msg)
    })
  rust.set_ack_callback(link, ack_callback)

  obs_callback = ffi.Callback('void', ['pointer', 'uint32', 'pointer', 'uint'],
    function(routePtr, prn, dataPtr, size) {
      let data = ref.reinterpret(dataPtr, size)
      let route = ref.reinterpret(routePtr, 17 * 4)

      let translatedRoute = route_to_arr(route)

      let msg = {
        'msg': data.toString(),
        'prn': prn,
        'route': translatedRoute
      }
      console.log("Obs ")
      console.log(msg)
      mainWindow.send('observe', msg)
    })
  rust.set_observe_callback(link, obs_callback)

  expire_callback = ffi.Callback('void', ['uint32'],
    function(prn) {
      mainWindow.send('expire', prn)
    })
  rust.set_expire_callback(link, expire_callback)

  retry_callback = ffi.Callback('void', ['uint32', 'uint32'],
    function(prn, next_retry) {
      mainWindow.send('retry', { 'prn': prn, 'next_retry': next_retry })
    })
  rust.set_retry_callback(link, retry_callback)

  if(msg.target == "loopback") {
    rust.open_loopback(link)
  } else {
    rust_serial.open_port(link, msg.target, 0)
  }

  tick = setInterval(() => {
    let now = Date.now()
    let elapsed = now - last_update

    last_update = now

    rust.tick(link, elapsed)
  }, 33)
})

function createWindow () {
  // Create the browser window.
  mainWindow = new BrowserWindow({width: 800, height: 600})

  // and load the index.html of the app.
  mainWindow.loadURL(url.format({
    pathname: path.join(__dirname, 'index.html'),
    protocol: 'file:',
    slashes: true
  }))

  mainWindow.webContents.once('did-finish-load', () => {
  })
  // Open the DevTools.
  //mainWindow.webContents.openDevTools()

  // Emitted when the window is closed.
  mainWindow.on('closed', function () {
    // Dereference the window object, usually you would store windows
    // in an array if your app supports multi windows, this is the time
    // when you should delete the corresponding element.
    mainWindow = null
  })
}

// This method will be called when Electron has finished
// initialization and is ready to create browser windows.
// Some APIs can only be used after this event occurs.
app.on('ready', createWindow)

// Quit when all windows are closed.
app.on('window-all-closed', function () {
  // On OS X it is common for applications and their menu bar
  // to stay active until the user quits explicitly with Cmd + Q
  if (process.platform !== 'darwin') {
    app.quit()
  }

  if(link != null) {
    rust.release(link)
    clearInterval(tick)
  }
})

app.on('activate', function () {
  // On OS X it's common to re-create a window in the app when the
  // dock icon is clicked and there are no other windows open.
  if (mainWindow === null) {
    createWindow()
  }
})

// In this file you can include the rest of your app's specific main process
// code. You can also put them in separate files and require them here.
