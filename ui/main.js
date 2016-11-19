const electron = require('electron')
// Module to control application life.
const app = electron.app
// Module to create native browser window.
const BrowserWindow = electron.BrowserWindow

const path = require('path')
const url = require('url')

const ffi = require('ffi')
const ref = require('ref')

var rust = ffi.Library("../capi/target/debug/slink_capi.dll", {
  'new': ['pointer', ['uint32'] ],
  'open_port': ['bool', ['pointer', 'string', 'uint'] ],
  'open_loopback': ['bool', ['pointer'] ],
  'close': ['void', ['pointer']],
  'tick' : ['bool', ['pointer', 'uint'] ],
  'send' : ['bool', ['pointer', 'pointer', 'pointer', 'uint'] ],
  'release' : ['void', ['pointer'] ],
  'set_recv_callback' : ['void', ['pointer', 'pointer'] ],
  'set_ack_callback' : ['void', ['pointer', 'pointer'] ],
  'set_expire_callback' : ['void', ['pointer', 'pointer'] ],
  'set_retry_callback' : ['void', ['pointer', 'pointer'] ],
  'set_observe_callback' : ['void', ['pointer', 'pointer'] ],
  'str_to_addr' : ['uint32', ['string'] ],
  'addr_to_str' : ['void', ['uint32', 'pointer']]
})

function addr_to_str(addr) {
  var buffer = Buffer.alloc(7)
  rust.addr_to_str(addr, buffer)

  return buffer.toString()
}

function route_to_arr(route) {
  var translatedRoute = []
  for(var i = 0; i < 17; ++i) {
    let translated = addr_to_str(route.readInt32LE(i*4))
    translatedRoute.push(translated)
  }

  return translatedRoute
}

// Keep a global reference of the window object, if you don't, the window will
// be closed automatically when the JavaScript object is garbage collected.
let mainWindow

let addr = rust.str_to_addr("KI7EST")
let link = rust.new(addr)

var recv_callback = ffi.Callback('void', ['uint32*', 'uint32', 'char*', 'uint'],
  function(routePtr, prn, dataPtr, size) {
    let data = ref.reinterpret(dataPtr, size)
    let route = ref.reinterpret(routePtr, 17 * 4)

    let translatedRoute = route_to_arr(route)

    console.log("Recv ")
    console.log({
      'data': data.toString(),
      'prn': prn,
      'route': translatedRoute
    })
  })
rust.set_recv_callback(link, recv_callback)

var ack_callback = ffi.Callback('void', ['pointer', 'uint32'],
  function(routePtr, prn) {
    let route = ref.reinterpret(routePtr, 17 * 4)
    let translatedRoute = route_to_arr(route)

    console.log("Ack");
    console.log({
      'prn': prn,
      'route': translatedRoute
    })
  })
rust.set_ack_callback(link, ack_callback)

var obs_callback = ffi.Callback('void', ['pointer', 'uint32', 'pointer', 'uint'],
  function(routePtr, prn, dataPtr, size) {
    let data = ref.reinterpret(dataPtr, size)
    let route = ref.reinterpret(routePtr, 17 * 4)

    var translatedRoute = route_to_arr(route)

    console.log("Obs ")
    console.log({
      'data': data.toString(),
      'prn': prn,
      'route': translatedRoute
    })
  })
rust.set_observe_callback(link, obs_callback)

var expire_callback = ffi.Callback('void', ['uint32'],
  function(prn) {
  })
rust.set_expire_callback(link, expire_callback)

var retry_callback = ffi.Callback('void', ['uint32'],
  function(prn) {
  })
rust.set_retry_callback(link, retry_callback)

rust.open_loopback(link)

{
  let msg = Buffer.from("Foo")
  let route = Buffer.alloc(4 * 15)
  route.writeUInt32LE(addr)

  rust.send(link, route, msg, msg.length)
}

rust.tick(link, 0)

function createWindow () {
  // Create the browser window.
  mainWindow = new BrowserWindow({width: 800, height: 600})

  // and load the index.html of the app.
  mainWindow.loadURL(url.format({
    pathname: path.join(__dirname, 'index.html'),
    protocol: 'file:',
    slashes: true
  }))

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
    //Pin the callback so we don't GC it
    recv_callback
    ack_callback
    obs_callback
    expire_callback
    retry_callback

    app.quit()
  }

  rust.release(link)
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
