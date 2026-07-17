sequenceDiagram
    participant "ExtractorControllerImpl" as p0
    participant "Codelist" as p1
    participant "OrquestrationLogServiceImpl" as p2
    participant "ParametersRepository" as p3
    participant "RulesServicesImpl" as p4
    participant "Additionalbusinesspartner" as p5
    participant "RulesRepository" as p6
    participant "AguiColumnsRecordDTO" as p7
    participant "ExcelElectricTypeRowMapper" as p8
    participant "Description" as p9
    participant "CategoriesMappingRepository" as p10

    activate p0
    p0->>p1: getCode()
    activate p1
    p1-->>p0: String
    deactivate p1
    p0->>p2: addNewLog()
    activate p2
    p2-->>p0: OrquestrationLog
    deactivate p2
    p0->>p2: updateLog()
    activate p2
    p2-->>p0: OrquestrationLog
    deactivate p2
    p0->>p2: addNewMessagelog()
    activate p2
    p2->>p5: setId()
    activate p5
    p5-->>p2: setId
    deactivate p5
    p2->>p5: setId()
    activate p5
    p5-->>p2: setId
    deactivate p5
    p2-->>p0: OrquestrationLogMessage
    deactivate p2
    p0->>p3: findByCountryAndEnvironmentAndName()
    activate p3
    p3-->>p0: Parameters
    deactivate p3
    p0->>p4: getRulesReadyToBeExecutedForDocs()
    activate p4
    p4->>p6: findRulesReadyToBeExecutedForDocs()
    activate p6
    p6-->>p4: List<Object>
    deactivate p6
    p4->>p6: findRulesReadyToBeExecutedWithCountryForDocs()
    activate p6
    p6-->>p4: List<Object>
    deactivate p6
    p4->>p4: getListOfEntiies()
    activate p4
    p4-->>p4: List<String>
    deactivate p4
    p4-->>p0: List<Object>
    deactivate p4
    p0->>p4: haveRuleSomeErrorAmsForDocs()
    activate p4
    p4->>p6: countErrorAmsInRuleForDocs()
    activate p6
    p6-->>p4: Long
    deactivate p6
    p4-->>p0: Long
    deactivate p4
    p0->>p0: convertObjectsToRules()
    activate p0
    p0->>p5: setId()
    activate p5
    p5-->>p0: setId
    deactivate p5
    p0->>p7: setTipoComp()
    activate p7
    p7-->>p0: setTipoComp
    deactivate p7
    p0->>p5: setId()
    activate p5
    p5-->>p0: setId
    deactivate p5
    p0->>p8: setDistributionCompany()
    activate p8
    p8-->>p0: setDistributionCompany
    deactivate p8
    p0->>p5: setId()
    activate p5
    p5-->>p0: setId
    deactivate p5
    p0->>p9: setDescription()
    activate p9
    p9-->>p0: setDescription
    deactivate p9
    p0-->>p0: List<Rules>
    deactivate p0
    p0->>p0: getRulesMap()
    activate p0
    p0->>p4: getListOfEntiies()
    activate p4
    p4-->>p0: List<String>
    deactivate p4
    p0->>p0: getAinCategoriesList()
    activate p0
    p0->>p4: getAinCategoriesListByAguiCategoryAndCountryAndEntityType()
    activate p4
    p4->>p10: getOnlyAinCategoriesByAguiCategoryAndCountryAndEntityType()
    activate p10
    p10-->>p4: List<String>
    deactivate p10
    p4-->>p0: List<String>
    deactivate p4
    p0-->>p0: List<String>
    deactivate p0
    p0-->>p0: result
    deactivate p0
    p0->>p0: getMaxCategoriesToRun()
    activate p0
    p0->>p0: getMaxNumberOfCategoriesToExtract()
    activate p0
    p0->>p3: findByCountryAndEnvironmentAndName()
    activate p3
    p3-->>p0: Parameters
    deactivate p3
    p0-->>p0: int
    deactivate p0
    p0-->>p0: int
    deactivate p0
    p0->>p0: getResultForStarter()
    activate p0
    p0-->>p0: JsonObject
    deactivate p0
    deactivate p0
