module InitPage exposing (..)

import Html exposing(Html, div, button, text, input)
import Html.Events exposing (onClick, onInput)

type alias Model = String

type Msg = SetLoopback
    | SetCom String
    | SetComSource String

model : Model
model = ""

update : Model -> Msg -> Model
update model msg =
    case msg of
        SetComSource(str) -> str
        _ -> model

view : Model -> (Msg -> a) -> Html a
view model conv =
    div [] [
        button [ onClick (conv SetLoopback) ] [ text "Loopback" ],
        button [ onClick (conv (SetCom model)) ] [ text "Com" ],
        input [ onInput (\str -> conv (SetComSource str)) ] []
    ]