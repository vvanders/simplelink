port module Main exposing (..)

import Html exposing (Html, button, div, text)
import Html.Events exposing (onClick)

main : Program Never Model Msg
main =
  Html.program {
    init = model,
    view = view,
    update = update,
    subscriptions = subscriptions
  }

-- MODEL
type alias Model = Int

model : (Model, Cmd Msg)
model =
  (0, Cmd.none) 

-- UPDATE
type Msg = Increment
  | Decrement
  | Retry Int
  | Expire Int
  | Ack AckMsg
  | Recv RecvMsg
  | Observe RecvMsg

update : Msg -> Model -> (Model, Cmd Msg)
update msg model =
  case msg of
    Increment ->
      (model + 1, Cmd.none)

    Decrement ->
      (model - 1, Cmd.none)

    _ ->
      (model, Cmd.none)

-- VIEW
view : Model -> Html Msg
view model =
  div []
    [ button [ onClick Decrement ] [ text "-" ]
    , div [] [ text (toString model) ]
    , button [ onClick Increment ] [ text "+" ]
    ]

-- OUTGOING
port simplelink_init : String -> Cmd msg

type alias SendMsg = {
  route : List String,
  msg : String
}
port simplelink_send : SendMsg -> Cmd msg

-- INCOMING
type alias RecvMsg = {
  route : List String,
  prn : Int,
  msg : String
}

type alias AckMsg = {
  route : List String,
  prn: Int
}

port simplelink_recv_msg : (RecvMsg -> msg) -> Sub msg
port simplelink_obs_msg : (RecvMsg -> msg) -> Sub msg
port simplelink_ack : (AckMsg -> msg) -> Sub msg
port simplelink_retry : (Int -> msg) -> Sub msg
port simplelink_expire : (Int -> msg) -> Sub msg

-- SUBSCRIPTIONS
subscriptions : Model -> Sub Msg
subscriptions model =
  Sub.batch [
    simplelink_retry Retry,
    simplelink_expire Expire,
    simplelink_ack Ack,
    simplelink_recv_msg Recv,
    simplelink_obs_msg Observe
  ]