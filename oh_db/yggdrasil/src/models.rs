// Data model:
//   Directory>    id:bigint | parent_id:bigint
//   File>         id:bigint | data:text | owner_dir:bigint(fk,notnull)
//   Formula>      id:bigint | fomula:text | owner_dir:bigint(fk,notnull)
//   FormulaInput> id:bigint | name:text | path:text | formula:bigint(fk,notnull)