port module Main exposing (..)

import InitPage
import MainPage
import SimpleLink exposing (..)

import Html exposing (Html, button, div, text)

main : Program Never Model Msg
main =
  Html.program {
    init = model,
    view = view,
    update = update,
    subscriptions = subscriptions
  }

-- MODEL
type Model = Init(InitPage.Model)
 | Main(MainPage.Model)

model : (Model, Cmd Msg)
model =
  (Init(InitPage.model), Cmd.none) 

-- UPDATE
type Msg = InitAction(InitPage.Msg) 
  | MainAction(MainPage.Msg)

update : Msg -> Model -> (Model, Cmd Msg)
update msg model =
  case msg of
    InitAction(InitPage.SetLoopback(callsign)) ->
      (Main(MainPage.init), simplelink_init { callsign = callsign, target = "loopback" })
    InitAction(InitPage.SetCom { target, callsign }) -> 
      (Main(MainPage.init), simplelink_init { callsign = callsign, target = target })
    InitAction(action) ->
      case model of
        Init(initModel) -> (Init(InitPage.update initModel action), Cmd.none)
        _ -> (model, Cmd.none)
    MainAction(action) ->
      case model of
        Main(mainModel) ->
          let
            (modelRes, cmdRes) = MainPage.update action mainModel simplelink_send
          in
            (Main(modelRes), Cmd.map (\cmd -> MainAction cmd) cmdRes)
        _ -> (model, Cmd.none)

-- VIEW
view : Model -> Html Msg
view model =
  let
    inner = case model of
      Init(initModel) -> InitPage.view initModel (\msg -> InitAction(msg))
      Main(mainModel) -> MainPage.view mainModel (\msg -> MainAction(msg))
  in
    div []
     [ inner ]

-- OUTGOING
port simplelink_init : { target : String, callsign : String } -> Cmd msg
port simplelink_send : SendMsg -> Cmd msg

-- INCOMING
port simplelink_recv_msg : (RecvMsg -> msg) -> Sub msg
port simplelink_obs_msg : (RecvMsg -> msg) -> Sub msg
port simplelink_send_msg : (SendMsg -> msg) -> Sub msg
port simplelink_ack : (AckMsg -> msg) -> Sub msg
port simplelink_retry : (PRN -> msg) -> Sub msg
port simplelink_expire : (PRN -> msg) -> Sub msg

-- SUBSCRIPTIONS
dispatch_link : (a -> SimpleLink.Msg) -> (a -> Msg)
dispatch_link msg =
  (\data -> MainAction(MainPage.Link(msg data)))

subscriptions : Model -> Sub Msg
subscriptions model =
  Sub.batch [
    simplelink_retry (dispatch_link SimpleLink.Retry),
    simplelink_expire (dispatch_link SimpleLink.Expire),
    simplelink_ack (dispatch_link SimpleLink.Ack),
    simplelink_recv_msg (dispatch_link SimpleLink.Recv),
    simplelink_obs_msg (dispatch_link SimpleLink.Observe),
    simplelink_send_msg (dispatch_link SimpleLink.Send)
  ]