typeset -g _anylist_query='' _anylist_list_name='' _anylist_typed='' _anylist_preview=''
typeset -ga _anylist_labels=() _anylist_choices=() _anylist_aliases=()
typeset -gi _anylist_show_menu=0 _anylist_owns_display=0 _anylist_focus=0
typeset -gi _anylist_query_requested=0
typeset -gA _anylist_previous

_anylist_reset_candidates() {
  _anylist_query=''
  _anylist_query_requested=0
  _anylist_show_menu=0
  _anylist_labels=() _anylist_choices=() _anylist_aliases=()
}

_anylist_get_candidates() {
  local query=$1 result row item_id alias name
  local tab=$'\t'
  _anylist_reset_candidates
  _anylist_query=$query
  _anylist_query_requested=1
  result=$(command anylist candidates -- "$query" 2>/dev/null) || return 0
  local -a lines=("${(@f)result}")
  _anylist_list_name=$lines[1]
  _anylist_show_menu=1
  for row in "${(@)lines[2,-1]}"; do
    item_id=${row%%$tab*}
    row=${row#*$tab}
    alias=${row%%$tab*}
    row=${row#*$tab}
    name=${row%%$tab*}
    row=${row#*$tab}
    _anylist_labels+=("$name  |  $row${alias:+  [$alias]}")
    _anylist_choices+=("${alias:-$item_id}")
    [[ -n $alias ]] && _anylist_aliases+=("$alias")
  done
  return 0
}

_anylist_clear_display() {
  if (( _anylist_owns_display )); then
    POSTDISPLAY=''
    _anylist_owns_display=0
  fi
}

_anylist_redraw() {
  emulate -L zsh
  setopt extendedglob
  _anylist_clear_display
  [[ $KEYMAP == vicmd ]] && return 0
  local query=''
  if (( _anylist_focus )); then
    if [[ $BUFFER != "$_anylist_preview" ]]; then
      BUFFER=$_anylist_preview
      CURSOR=$#BUFFER
      zle beep
    fi
  elif [[ $BUFFER == 'anylist untick '* ]]; then
    _anylist_typed=$BUFFER
    query=${_anylist_typed#anylist untick }
  fi
  (( _anylist_focus )) && query=$_anylist_query
  query=${query##[[:space:]]#}
  query=${query%%[[:space:]]#}
  if [[ -z $query ]]; then
    [[ -z $_anylist_query ]] || _anylist_reset_candidates
    return 0
  fi
  [[ $query == "$_anylist_query" ]] || _anylist_get_candidates "$query"
  (( _anylist_show_menu )) || return 0
  local -a rows=()
  local marker
  local -i index
  for (( index = 1; index <= $#_anylist_labels; index++ )); do
    marker=' '
    (( index == _anylist_focus )) && marker='>'
    rows+=("$marker ${_anylist_labels[index]}")
  done
  if (( ! $#rows )); then
    rows+=("No matching item in '$_anylist_list_name'")
  fi
  POSTDISPLAY=$'\n'"${(F)rows}"
  _anylist_owns_display=1
}

_anylist_line_init() {
  _anylist_clear_display
  _anylist_focus=0
  _anylist_reset_candidates
}

_anylist_preview_choice() {
  local choice=${_anylist_choices[_anylist_focus]}
  BUFFER="anylist untick ${(q)choice}"
  CURSOR=$#BUFFER
  _anylist_preview=$BUFFER
}

_anylist_restore() {
  _anylist_focus=0
  BUFFER=$_anylist_typed
  CURSOR=$#BUFFER
}

_anylist_delegate() {
  local previous=${_anylist_previous[$WIDGET]}
  [[ $previous == undefined-key ]] && return 0
  zle "$previous" "$@"
}

_anylist_down() {
  [[ $KEYMAP == vicmd || $BUFFER != 'anylist untick '* ]] && { _anylist_delegate "$@"; return; }
  _anylist_redraw
  if (( _anylist_focus < $#_anylist_choices )); then
    (( ++_anylist_focus ))
    _anylist_preview_choice
  fi
}

_anylist_up() {
  if [[ $KEYMAP == vicmd ]] || (( ! _anylist_focus )); then
    _anylist_delegate "$@"
  elif (( _anylist_focus == 1 )); then
    _anylist_restore
  else
    (( --_anylist_focus ))
    _anylist_preview_choice
  fi
}

_anylist_escape() {
  if [[ $KEYMAP != vicmd ]] && (( _anylist_focus )); then
    _anylist_restore
  else
    _anylist_delegate "$@"
  fi
}

_anylist_enter() {
  if [[ $KEYMAP == vicmd || $BUFFER != anylist && $BUFFER != 'anylist '* ]]; then
    _anylist_delegate "$@"
  else
    _anylist_clear_display
    _anylist_focus=0
    zle .accept-line
  fi
}

_anylist_tab() {
  emulate -L zsh
  if [[ $KEYMAP == vicmd || $BUFFER != 'anylist '* ]]; then
    _anylist_delegate "$@"
    return
  fi
  if (( _anylist_focus )); then
    zle beep
    return 0
  fi

  local stem='anylist ' prefix candidate completion
  local -a choices=("${_anylist_commands[@]}") matches=()
  if [[ $BUFFER == 'anylist untick '* ]]; then
    stem='anylist untick '
    _anylist_redraw
    if [[ $BUFFER == "$stem" ]] && (( ! _anylist_query_requested )); then
      _anylist_get_candidates ''
    fi
    choices=("${_anylist_aliases[@]}")
  fi
  prefix=${BUFFER#"$stem"}
  for candidate in "${choices[@]}"; do
    [[ $candidate == "$prefix"* ]] && matches+=("$candidate")
  done
  (( $#matches )) || return 0
  completion=$matches[1]
  for candidate in "${matches[@]}"; do
    while [[ $candidate != "$completion"* ]]; do
      completion=${completion%?}
    done
  done
  if [[ $stem == 'anylist ' ]] && (( $#matches == 1 )); then
    completion+=' '
  elif [[ $stem == 'anylist untick ' ]] && (( $#matches == 1 )); then
    completion=${(q)completion}
  fi
  BUFFER="$stem$completion"
  CURSOR=$#BUFFER
}

_anylist_bind() {
  emulate -L zsh
  local key=$1 action=$2 widget="_anylist_key_$3"
  local -a binding
  binding=(${(z)"$(bindkey -M main "$key")"})
  local previous=${(Q)binding[-1]}
  [[ $binding[-1] == \"* ]] && previous=undefined-key
  [[ $previous == "$widget" ]] && return 0
  _anylist_previous[$widget]=$previous
  zle -N "$widget" "$action"
  bindkey -M main "$key" "$widget"
}

autoload -Uz add-zle-hook-widget
add-zle-hook-widget line-pre-redraw _anylist_redraw
add-zle-hook-widget line-init _anylist_line_init
_anylist_bind '^M' _anylist_enter 1
_anylist_bind '^J' _anylist_enter 2
_anylist_bind '^[[A' _anylist_up 3
_anylist_bind '^[[B' _anylist_down 4
_anylist_bind '^[OA' _anylist_up 5
_anylist_bind '^[OB' _anylist_down 6
_anylist_bind '^[' _anylist_escape 7
_anylist_bind '^I' _anylist_tab 8
