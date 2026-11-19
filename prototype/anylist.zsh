typeset -g _anylist_demo_list_name='queste sono le cose'
typeset -ga _anylist_demo_ids=(
  4f1d2c9a8b7e4e3fa61c0d5b2e9f7a13 9c3e7b21d5a84f60b2e1c7d94a6f0e85
  2b8f6e0c1a9d4c7eb53f8a2d6e1c0b97 d07a5c3e9f2b4a18c6e0b7f1d3a9e542
  6e2a9f1b4c8d4d05a7b3e9c0f2d6b871 a1c7e4b9d2f04e6b8c5a3f0e7d1b9c26
  58d3b0e6f9a14c2d9e7b1a4c0f8e6d3b
)
typeset -gA _anylist_demo_names=(
  4f1d2c9a8b7e4e3fa61c0d5b2e9f7a13 'Milk'
  9c3e7b21d5a84f60b2e1c7d94a6f0e85 'Oat milk'
  2b8f6e0c1a9d4c7eb53f8a2d6e1c0b97 'Coffee beans'
  d07a5c3e9f2b4a18c6e0b7f1d3a9e542 'Coffee beans'
  6e2a9f1b4c8d4d05a7b3e9c0f2d6b871 'Earl Grey tea'
  a1c7e4b9d2f04e6b8c5a3f0e7d1b9c26 'Cream cheese'
  58d3b0e6f9a14c2d9e7b1a4c0f8e6d3b 'Bagels'
)
typeset -gA _anylist_demo_notes=(
  4f1d2c9a8b7e4e3fa61c0d5b2e9f7a13 'Organic, whole'
  9c3e7b21d5a84f60b2e1c7d94a6f0e85 'Unsweetened'
  2b8f6e0c1a9d4c7eb53f8a2d6e1c0b97 'Whole bean'
  d07a5c3e9f2b4a18c6e0b7f1d3a9e542 'Whole bean'
  6e2a9f1b4c8d4d05a7b3e9c0f2d6b871 "Sam's favourite; bergamot"
  a1c7e4b9d2f04e6b8c5a3f0e7d1b9c26 'Philadelphia'
  58d3b0e6f9a14c2d9e7b1a4c0f8e6d3b "For Phil's brunch"
)
# Hard-coded stand-ins for user-created aliases; alias creation is not prototyped.
typeset -gA _anylist_demo_aliases=(
  9c3e7b21d5a84f60b2e1c7d94a6f0e85 oatly
  6e2a9f1b4c8d4d05a7b3e9c0f2d6b871 earl
  a1c7e4b9d2f04e6b8c5a3f0e7d1b9c26 philly
)
typeset -gA _anylist_demo_alias_ids=()
typeset -gA _anylist_demo_checked=()
typeset -gA _anylist_demo_labels=()
for _anylist_demo_id in $_anylist_demo_ids; do
  _anylist_demo_checked[$_anylist_demo_id]=1
  _anylist_demo_labels[$_anylist_demo_id]="${_anylist_demo_names[$_anylist_demo_id]}  |  ${_anylist_demo_notes[$_anylist_demo_id]}"
  if (( ${+_anylist_demo_aliases[$_anylist_demo_id]} )); then
    _anylist_demo_alias_ids[${_anylist_demo_aliases[$_anylist_demo_id]}]=$_anylist_demo_id
    _anylist_demo_labels[$_anylist_demo_id]+="  [${_anylist_demo_aliases[$_anylist_demo_id]}]"
  fi
done
unset _anylist_demo_id
typeset -ga _anylist_demo_matches=()
typeset -gi _anylist_demo_focus=0 _anylist_demo_active=0 _anylist_demo_accepting=0
typeset -g _anylist_demo_typed=''
typeset -g _anylist_demo_preview_buffer=''

anylist() {
  emulate -L zsh
  if [[ $# != 2 || $1 != untick ]]; then
    print -u2 -r -- 'Usage: anylist untick <alias|item_id>'
    return 2
  fi
  local item_id=${_anylist_demo_alias_ids[$2]:-$2}
  if (( ! ${+_anylist_demo_names[$item_id]} )); then
    print -u2 -r -- "No item with alias or ID '$2' in $_anylist_demo_list_name"
    return 1
  fi
  _anylist_demo_checked[$item_id]=0
  print -r -- "DEMO: Unticked: ${_anylist_demo_names[$item_id]} ($item_id) on $_anylist_demo_list_name"
}

_anylist_demo_find() {
  emulate -L zsh
  setopt extendedglob
  local query=$1 item_id searchable
  query=${query##[[:space:]]#}
  query=${query%%[[:space:]]#}
  query=${(L)query}
  _anylist_demo_matches=()
  [[ -n $query ]] || return 1
  for item_id in $_anylist_demo_ids; do
    searchable="${_anylist_demo_names[$item_id]} ${_anylist_demo_notes[$item_id]} ${_anylist_demo_aliases[$item_id]} $item_id"
    if [[ ${(L)searchable} == *"$query"* ]]; then
      _anylist_demo_matches+=($item_id)
    fi
  done
  return 0
}

_anylist_demo_refresh() {
  emulate -L zsh
  POSTDISPLAY=''
  _anylist_demo_active=0
  (( _anylist_demo_accepting )) && return
  # A previewed command is read-only; leave the list with Esc or Up to edit.
  if (( _anylist_demo_focus )) && [[ $BUFFER != $_anylist_demo_preview_buffer ]]; then
    BUFFER=$_anylist_demo_preview_buffer
    CURSOR=$#BUFFER
    zle beep
  fi
  if [[ $BUFFER != 'anylist untick '* ]]; then
    _anylist_demo_focus=0
    return
  fi
  _anylist_demo_active=1
  (( _anylist_demo_focus )) || _anylist_demo_typed=${BUFFER#anylist untick }
  _anylist_demo_find "$_anylist_demo_typed" || return
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

_anylist_demo_preview() {
  local item_id=$_anylist_demo_matches[$_anylist_demo_focus]
  BUFFER="anylist untick ${(q)${_anylist_demo_aliases[$item_id]:-$item_id}}"
  CURSOR=$#BUFFER
  _anylist_demo_preview_buffer=$BUFFER
}

_anylist_demo_restore() {
  _anylist_demo_focus=0
  BUFFER="anylist untick $_anylist_demo_typed"
  CURSOR=$#BUFFER
}

_anylist_demo_tab() {
  emulate -L zsh
  setopt extendedglob
  _anylist_demo_refresh
  if [[ $BUFFER == anylist\ * && 'anylist untick ' == $BUFFER* ]]; then
    BUFFER='anylist untick '
    CURSOR=$#BUFFER
    return
  fi
  if (( ! _anylist_demo_active )); then
    zle expand-or-complete
    return
  fi
  local prefix=${BUFFER#anylist untick } completion candidate
  local -a candidates=(${_anylist_demo_alias_ids[(I)${~${(b)prefix}}*]})
  (( $#candidates )) || return 0
  # Extend to the longest prefix shared by every candidate alias.
  completion=$candidates[1]
  for candidate in $candidates; do
    while [[ $candidate != $completion* ]]; do
      completion=${completion%?}
    done
  done
  BUFFER="anylist untick $completion"
  CURSOR=$#BUFFER
}

_anylist_demo_enter() {
  emulate -L zsh
  _anylist_demo_accepting=1
  POSTDISPLAY=''
  zle .accept-line
}

_anylist_demo_line_init() {
  _anylist_demo_active=0
  _anylist_demo_accepting=0
  _anylist_demo_matches=()
  _anylist_demo_focus=0
  _anylist_demo_typed=''
  _anylist_demo_preview_buffer=''
  POSTDISPLAY=''
}

_anylist_demo_up() {
  if (( _anylist_demo_active && _anylist_demo_focus > 1 )); then
    (( --_anylist_demo_focus ))
    _anylist_demo_preview
  elif (( _anylist_demo_active && _anylist_demo_focus == 1 )); then
    _anylist_demo_restore
  else
    zle up-line-or-history
  fi
}

_anylist_demo_down() {
  if (( _anylist_demo_active )); then
    if (( _anylist_demo_focus < $#_anylist_demo_matches )); then
      (( ++_anylist_demo_focus ))
      _anylist_demo_preview
    fi
    return 0
  fi
  zle down-line-or-history
}

_anylist_demo_escape() {
  (( _anylist_demo_active && _anylist_demo_focus )) && _anylist_demo_restore
  return 0
}

autoload -Uz add-zle-hook-widget
zle -N _anylist_demo_refresh
zle -N _anylist_demo_tab
zle -N _anylist_demo_enter
zle -N _anylist_demo_line_init
zle -N _anylist_demo_up
zle -N _anylist_demo_down
zle -N _anylist_demo_escape
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
bindkey '^[' _anylist_demo_escape
print -r -- 'AnyList interaction tracer loaded. Sample data only; no network requests.'
print -r -- 'Type: anylist untick phil   (Down previews, Esc restores, Enter runs the line)'
print -r -- 'Tab completes the subcommand and aliases (philly, oatly, earl).'
print -r -- 'Also try: sweet, bean, bergamot. Ctrl-C cancels. Exit leaves the demo shell.'
