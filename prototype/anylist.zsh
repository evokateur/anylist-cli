typeset -g _anylist_demo_list_name='queste sono le cose'
typeset -ga _anylist_demo_ids=(milk oat-milk beans-a beans-b tea)
typeset -gA _anylist_demo_names=(
  milk 'Milk' oat-milk 'Oat milk' beans-a 'Coffee beans'
  beans-b 'Coffee beans' tea "Earl Grey tea"
)
typeset -gA _anylist_demo_notes=(
  milk 'Organic, whole' oat-milk 'Unsweetened' beans-a 'Whole bean'
  beans-b 'Whole bean' tea "Sam's favourite; bergamot"
)
typeset -gA _anylist_demo_checked=(milk 1 oat-milk 1 beans-a 1 beans-b 1 tea 1)
typeset -gA _anylist_demo_labels=()
for _anylist_demo_id in $_anylist_demo_ids; do
  _anylist_demo_labels[$_anylist_demo_id]="${_anylist_demo_names[$_anylist_demo_id]}  |  ${_anylist_demo_notes[$_anylist_demo_id]}  [$_anylist_demo_id]"
done
unset _anylist_demo_id
typeset -ga _anylist_demo_matches=()
typeset -gi _anylist_demo_focus=1 _anylist_demo_active=0
typeset -g _anylist_demo_query=''
typeset -g _anylist_demo_completed_buffer=''

anylist() {
  emulate -L zsh
  if [[ $# != 2 || $1 != untick ]]; then
    print -u2 -r -- 'Usage: anylist untick <item_id>'
    return 2
  fi
  local item_id=$2
  if (( ! ${+_anylist_demo_names[$item_id]} )); then
    print -u2 -r -- "No item with ID '$item_id' in $_anylist_demo_list_name"
    return 1
  fi
  _anylist_demo_checked[$item_id]=0
  print -r -- "DEMO: Unticked: ${_anylist_demo_names[$item_id]} [$item_id] on $_anylist_demo_list_name"
  print -r -- "Resolved command: anylist untick ${(q)item_id}"
}

_anylist_demo_find() {
  emulate -L zsh
  setopt extendedglob
  local query=$1 item_id searchable
  query=${query##[[:space:]]#}
  query=${query%%[[:space:]]#}
  query=${(L)query}
  if [[ $query != $_anylist_demo_query ]]; then
    _anylist_demo_focus=1
  fi
  _anylist_demo_query=$query
  _anylist_demo_matches=()
  [[ -n $query ]] || return 0
  for item_id in $_anylist_demo_ids; do
    searchable="${_anylist_demo_names[$item_id]} ${_anylist_demo_notes[$item_id]} $item_id"
    if [[ ${(L)searchable} == *"$query"* ]]; then
      _anylist_demo_matches+=($item_id)
    fi
  done
  (( _anylist_demo_focus > $#_anylist_demo_matches )) && _anylist_demo_focus=1
  return 0
}

_anylist_demo_refresh() {
  emulate -L zsh
  POSTDISPLAY=''
  _anylist_demo_active=0
  if [[ -n $_anylist_demo_completed_buffer && $BUFFER == $_anylist_demo_completed_buffer ]]; then
    return
  fi
  _anylist_demo_completed_buffer=''
  if [[ $BUFFER != 'anylist untick '* ]]; then
    return
  fi
  _anylist_demo_active=1
  _anylist_demo_find "${BUFFER#anylist untick }"
  if [[ -z $_anylist_demo_query ]]; then
    return
  fi
  local -a rows=()
  local item_id marker
  local -i index=0
  for item_id in $_anylist_demo_matches; do
    (( ++index ))
    marker=' '
    (( index == _anylist_demo_focus )) && marker='>'
    rows+=("$marker ${_anylist_demo_labels[$item_id]}")
  done
  if (( ! $#_anylist_demo_matches )); then
    rows+=("No matching item in '$_anylist_demo_list_name'")
  fi
  POSTDISPLAY=$'\n'"${(F)rows}"
}

_anylist_demo_tab() {
  emulate -L zsh
  _anylist_demo_refresh
  if (( ! _anylist_demo_active )); then
    zle expand-or-complete
    return
  fi
  (( $#_anylist_demo_matches )) || return 0
  local item_id=$_anylist_demo_matches[$_anylist_demo_focus]
  BUFFER="anylist untick ${(q)item_id}"
  CURSOR=$#BUFFER
  _anylist_demo_completed_buffer=$BUFFER
  _anylist_demo_active=0
  POSTDISPLAY=''
}

_anylist_demo_enter() {
  emulate -L zsh
  _anylist_demo_refresh
  if (( _anylist_demo_active && $#_anylist_demo_matches )); then
    local item_id=$_anylist_demo_matches[$_anylist_demo_focus]
    BUFFER="anylist untick ${(q)item_id}"
    CURSOR=$#BUFFER
  fi
  _anylist_demo_completed_buffer=$BUFFER
  POSTDISPLAY=''
  zle .accept-line
}

_anylist_demo_line_init() {
  _anylist_demo_active=0
  _anylist_demo_matches=()
  _anylist_demo_focus=1
  _anylist_demo_query=''
  _anylist_demo_completed_buffer=''
  POSTDISPLAY=''
}

_anylist_demo_up() {
  if (( _anylist_demo_active )); then
    (( _anylist_demo_focus > 1 )) && (( --_anylist_demo_focus ))
    return 0
  fi
  zle up-line-or-history
}

_anylist_demo_down() {
  if (( _anylist_demo_active )); then
    (( _anylist_demo_focus < $#_anylist_demo_matches )) && (( ++_anylist_demo_focus ))
    return 0
  fi
  zle down-line-or-history
}

autoload -Uz add-zle-hook-widget
zle -N _anylist_demo_refresh
zle -N _anylist_demo_tab
zle -N _anylist_demo_enter
zle -N _anylist_demo_line_init
zle -N _anylist_demo_up
zle -N _anylist_demo_down
add-zle-hook-widget line-pre-redraw _anylist_demo_refresh
add-zle-hook-widget line-init _anylist_demo_line_init
bindkey -e
bindkey '^I' _anylist_demo_tab
bindkey '^M' _anylist_demo_enter
bindkey '^J' _anylist_demo_enter
bindkey '^[[A' _anylist_demo_up
bindkey '^[[B' _anylist_demo_down
bindkey '^[OA' _anylist_demo_up
bindkey '^[OB' _anylist_demo_down
print -r -- 'AnyList interaction tracer loaded. Sample data only; no network requests.'
print -r -- 'Type: anylist untick mil   (then Down optionally, Enter)'
print -r -- 'Tab completes anylist untick <item_id> without executing.'
print -r -- 'Also try: sweet, bean, beans-a, bergamot. Ctrl-C cancels. Exit leaves the demo shell.'
