module InitPage exposing (..)

import Html exposing(Html, div, button, text, input)
import Html.Events exposing (onClick, onInput)

type alias Model = { callsign : String, target : String }

type Msg = SetLoopback String
    | SetCom { callsign: String, target : String }
    | SetComSource String
    | SetCallsign String

model : Model
model = { target = "", callsign = "" }

update : Model -> Msg -> Model
update model msg =
    case msg of
        SetComSource(str) -> { model | target = str }
        SetCallsign(str) -> { model | callsign = str }
        SetCom(_) -> model
        SetLoopback(_) -> model

view : Model -> (Msg -> a) -> Html a
view model conv =
    div [] [
        div [] [
            text "Callsign: ",
            input [ onInput (\str -> conv (SetCallsign str))] []
        ],
        div [] [
            button [ onClick (conv (SetLoopback model.callsign)) ] [ text "Loopback" ],
            button [ onClick (conv (SetCom model)) ] [ text "Com" ],
            input [ onInput (\str -> conv (SetComSource str)) ] []
        ]
    ]